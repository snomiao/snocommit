//! What the runtime around the TypeScript acts on before or beside the
//! program: the cases where the binary cannot know what the JavaScript would
//! print, and so hands the invocation back before it looks any further.
//!
//! The commands' files start with `#!/usr/bin/env node`, and the hand-back
//! starts them the same way (js_fallback `runtimes`): with the `node` on
//! PATH, or with bun where there is none. Node reads nothing of the current
//! directory. bun, which also stands in for node where `node` is a link to
//! it, loads .env files, reads bunfig.toml, and reports on stderr a
//! package.json, tsconfig.json or jsconfig.json in the current directory or
//! above it that it cannot parse, or whose fields are not of the kind it
//! expects. Wherever the JavaScript might run under bun, any such file sends
//! the invocation back (a package.json only when it holds more than a few
//! plain fields).

use std::path::{Path, PathBuf};

/// Variables that make the JavaScript behave in ways the port does not
/// follow, by name. Any one of them, set to anything, hands back.
pub fn variable_hands_back(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    // bun's and node's own (BUN_INSTALL, which bun's installer sets for
    // nearly everyone, changes nothing here), and the allocator's and ICU's.
    (name.starts_with("BUN_") && !name.starts_with("BUN_INSTALL"))
        || name.starts_with("NODE_")
        || name.starts_with("MIMALLOC_")
        || name.starts_with("ICU_")
        // supports-color: the ones the port does not spell out.
        || matches!(name, "FORCE_COLOR" | "TF_BUILD" | "TEAMCITY_VERSION")
        // bun's fetch honours a proxy and other roots; node's does not.
        || matches!(upper.as_str(), "HTTP_PROXY" | "HTTPS_PROXY" | "ALL_PROXY" | "NO_PROXY")
        || matches!(name, "SSL_CERT_FILE" | "SSL_CERT_DIR")
        // The OpenAI SDK reads these (organization, project, logging, …).
        || (name.starts_with("OPENAI_") && !matches!(name, "OPENAI_API_KEY" | "OPENAI_BASE_URL"))
        // git writes traces (to a file, too) and starts an external diff for
        // every command, and the binary runs add, status and diff twice (on
        // its copy of the index, then for real); see commit::PROGRAMS for
        // the same in git's configuration.
        || name.starts_with("GIT_TRACE")
        || name == "GIT_EXTERNAL_DIFF"
}

/// The whole environment: every name and value valid UTF-8 (node decodes
/// them, and the programs it starts get them encoded again, so a byte that
/// is not UTF-8 would reach git changed), and none of the variables above.
pub fn environment_is_plain() -> bool {
    std::env::vars_os().all(|(name, value)| match (name.to_str(), value.to_str()) {
        (Some(name), Some(_)) => !variable_hands_back(name),
        _ => false,
    })
}

/// Whether the `node` that the command's `#!` line asks for is node itself:
/// the first file called `node` on PATH that can be run, which is not a
/// script, whose real name (after symbolic links) is `node`, and which, run,
/// says it is not bun. Anything else (no node, a link to bun, a shim script,
/// a hard link or a copy of bun under the name node) may run the JavaScript
/// under bun.
pub fn runtime_is_node(cwd: &Path) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("node");
        if !is_executable_file(&candidate) {
            continue;
        }
        let Ok(real) = std::fs::canonicalize(&candidate) else {
            return false;
        };
        if real.file_name().and_then(|n| n.to_str()) != Some("node") {
            return false;
        }
        let mut head = [0u8; 2];
        let read = std::fs::File::open(&real).and_then(|mut f| std::io::Read::read(&mut f, &mut head));
        return matches!(read, Ok(2)) && &head != b"#!" && says_it_is_node(&candidate, cwd);
    }
    false
}

/// What the JavaScript tells apart, `typeof Bun` and `process.versions.bun`,
/// asked of `node` itself where the command would run (bun under the name
/// node answers `object,string`, and does not care what its file is called).
/// Nothing is printed; a node that does not answer within ten seconds, or
/// answers anything else, counts as not node.
fn says_it_is_node(node: &Path, cwd: &Path) -> bool {
    use std::io::Read;
    let child = std::process::Command::new(node)
        .args(["-e", "process.stdout.write(typeof Bun + ',' + typeof process.versions.bun)"])
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .env_remove(crate::js_fallback::TRACE_ENV)
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    // Read on a thread of its own, so that neither a node that hangs nor one
    // that leaves something behind holding its stdout can hold the binary.
    let (tell, told) = std::sync::mpsc::channel();
    if let Some(mut out) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut answer = Vec::new();
            let read = out.by_ref().take(4096).read_to_end(&mut answer);
            let _ = tell.send(read.ok().map(|_| answer));
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let answer = told.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()));
    let mut status = None;
    while answer.is_ok() && std::time::Instant::now() < deadline {
        match child.try_wait() {
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(2)),
            done => {
                status = done.ok().flatten();
                break;
            }
        }
    }
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    status.is_some_and(|s| s.success()) && matches!(answer, Ok(Some(text)) if text == b"undefined,undefined")
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// What bun would read in `cwd` and above it, and in the user's own
/// configuration, is absent or of a kind bun reads without a word.
pub fn nothing_for_bun(cwd: &Path, home: Option<&Path>) -> bool {
    // .env files: the ones bun loads when NODE_ENV is unset (a NODE_
    // variable hands back before this).
    for name in [".env", ".env.local", ".env.development", ".env.development.local", "bunfig.toml"] {
        if exists(&cwd.join(name)) {
            return false;
        }
    }
    let mut configs: Vec<PathBuf> = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        configs.push(PathBuf::from(xdg).join(".bunfig.toml"));
    }
    if let Some(home) = home {
        configs.push(home.join(".bunfig.toml"));
    }
    if configs.iter().any(|path| exists(path)) {
        return false;
    }
    for dir in cwd.ancestors() {
        if exists(&dir.join("tsconfig.json")) || exists(&dir.join("jsconfig.json")) {
            return false;
        }
        let package = dir.join("package.json");
        if exists(&package) && !plain_package_json(&package) {
            return false;
        }
    }
    true
}

/// Anything at all at `path`, or something that cannot be looked at.
fn exists(path: &Path) -> bool {
    !matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

/// A package.json that is a regular file of strict JSON: one object whose
/// fields are only `name`, `version`, `description`, `license` and `author`
/// as strings and `private` as true or false. bun reads that without a word.
pub fn plain_package_json(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() > 64 * 1024 {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    plain_package_text(&text)
}

fn plain_package_text(text: &str) -> bool {
    let Ok(serde_json::Value::Object(fields)) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    fields.iter().all(|(key, value)| match key.as_str() {
        "name" | "version" | "description" | "license" | "author" => value.is_string(),
        "private" => value.is_boolean(),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables() {
        for name in ["BUN_OPTIONS", "NODE_OPTIONS", "NODE_ENV", "FORCE_COLOR", "http_proxy", "OPENAI_ORG_ID", "OPENAI_LOG", "ICU_DATA", "MIMALLOC_VERBOSE"] {
            assert!(variable_hands_back(name), "{name}");
        }
        for name in ["GIT_TRACE", "GIT_TRACE2_EVENT", "GIT_TRACE_PACKET", "GIT_EXTERNAL_DIFF"] {
            assert!(variable_hands_back(name), "{name}");
        }
        for name in ["BUN_INSTALL", "BUN_INSTALL_CACHE_DIR", "OPENAI_API_KEY", "OPENAI_BASE_URL", "NO_COLOR", "TERM", "HOME", "GIT_DIR", "GIT_INDEX_FILE"] {
            assert!(!variable_hands_back(name), "{name}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn bun_under_the_name_node_is_not_node() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("snocommit-test-node-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let script = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        assert!(says_it_is_node(&script("plain", "printf undefined,undefined"), &dir));
        assert!(!says_it_is_node(&script("bun", "printf object,string"), &dir));
        assert!(!says_it_is_node(&script("fails", "printf undefined,undefined; exit 1"), &dir));
        assert!(!says_it_is_node(&dir.join("none"), &dir));
        // bun itself, hard-linked (or copied) under the name node.
        let bun = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|d| d.join("bun")).find(|p| p.is_file());
        if let Some(bun) = bun.and_then(|b| std::fs::canonicalize(b).ok()) {
            let node = dir.join("node");
            if std::fs::hard_link(&bun, &node).is_err() {
                std::fs::copy(&bun, &node).unwrap();
            }
            assert!(!says_it_is_node(&node, &dir));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn package_json() {
        assert!(plain_package_text(r#"{"name":"x","version":"1.0.0","private":true}"#));
        assert!(plain_package_text("{}"));
        assert!(!plain_package_text(r#"{"name":"x","type":"module"}"#));
        assert!(!plain_package_text(r#"{"name":5}"#));
        assert!(!plain_package_text("[1]"));
        assert!(!plain_package_text(r#"{"name":"x",}"#));
        assert!(!plain_package_text(""));
    }
}
