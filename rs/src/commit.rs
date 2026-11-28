//! `feat <scope> <description…>` and the other five: stage by scope, show
//! the changes, ask the model for a message, commit, pull and push, echoing
//! every git command (src/index.tsx, src/cli.ts).
//!
//! The TypeScript stages first and finds out afterwards whether there was
//! anything to commit, which message the model proposes, and whether the
//! service answered at all; every failure after the staging ends it as an
//! unhandled rejection in the runtime's own words. The binary may hand back
//! only while it has printed and changed nothing, so it finds all of that
//! out first, on a copy of the index (`GIT_INDEX_FILE`): it stages into the
//! copy, reads the status and the diffs from it, reads the log, asks the
//! model once, and checks the answer. Only when every step is one it
//! carries out exactly does it go on (`Plan::carry_out`), and then it runs
//! the TypeScript's commands for real, in its order, with its output; the
//! status, the diffs and the log must come out as they did on the copy,
//! which they do unless something else changed the repository meanwhile.
//! The objects the copy's `git add` writes into the repository are the ones
//! the real `git add` writes; nothing else outside the copy is touched.
//!
//! Never given to the shell: a command line in which the shell would run,
//! substitute or redirect anything (a scope, a path in the status, the
//! model's message with `$`, a backquote or a backslash). The TypeScript
//! passes those to the shell as they are, which runs what they say; the
//! binary hands them back instead of doing the same (see the ledger).

use crate::js_fallback::HandBack;
use crate::openai::{self, TYPES};
use crate::{color, gate, js, sh};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Output larger than this from any git command run on the copy goes back,
/// counted as exec counts it (bytes of the decoded text): node's `exec`
/// kills a command past 1 MiB (sh::MAX_BUFFER), and the real run would then
/// be cut off where the copy's was not.
const MAX_OUTPUT: usize = 1_000_000;

/// The longest message the binary commits; a longer one goes back (a
/// command line has a limit, and what exec does past it is node's).
const MAX_MESSAGE: usize = 100_000;

const STATUS: &str = "git status --porcelain";
const LOG: &str = "git log -n 5 --pretty=format:\"%h\t%an\t%ad\t%s\" --date=short";
const PULL_PUSH: &str = "git pull && git push";

/// The type src/cli.ts finds in the path of its own file:
/// `/(?:cli-|bin\/)(.*?)(?:\.[mc]?[tj]s)?$/`, then one of the six.
pub fn type_from_script(path: &str) -> Option<&'static str> {
    // `.` does not match a line terminator, and the match would move on.
    if path.contains(js::is_line_terminator) {
        return None;
    }
    let start = (0..path.len()).find(|&at| path.is_char_boundary(at) && (path[at..].starts_with("cli-") || path[at..].starts_with("bin/")))?;
    let rest = &path[start + 4..];
    let ext = |s: &str| {
        let b = s.as_bytes();
        match b.len() {
            3 => b[0] == b'.' && matches!(b[1], b't' | b'j') && b[2] == b's',
            4 => b[0] == b'.' && matches!(b[1], b'm' | b'c') && matches!(b[2], b't' | b'j') && b[3] == b's',
            _ => false,
        }
    };
    let name = if rest.len() >= 4 && rest.is_char_boundary(rest.len() - 4) && ext(&rest[rest.len() - 4..]) {
        &rest[..rest.len() - 4]
    } else if rest.len() >= 3 && rest.is_char_boundary(rest.len() - 3) && ext(&rest[rest.len() - 3..]) {
        &rest[..rest.len() - 3]
    } else {
        rest
    };
    TYPES.iter().copied().find(|t| *t == name)
}

/// `path.resolve(path)` on POSIX: absolute, without `.`, `..` and repeated
/// or trailing slashes.
fn resolve(cwd: &str, path: &str) -> String {
    let joined = if path.starts_with('/') { path.to_string() } else { format!("{cwd}/{path}") };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

/// Whether the JavaScript, started with `script`, would take itself for
/// `command`: as node sees its path (`path.resolve`), and as bun does (the
/// real path).
fn script_says(script: &Path, cwd: &str, command: &str) -> bool {
    let Some(text) = script.to_str() else {
        return false;
    };
    if type_from_script(&resolve(cwd, text)) != Some(command) {
        return false;
    }
    match std::fs::canonicalize(script) {
        Ok(real) => real.to_str().and_then(type_from_script) == Some(command),
        Err(_) => true,
    }
}

/// A command line in which the shell runs, substitutes and redirects
/// nothing: none of `$ ` ; & | < > ( )`, no quotes or backslashes, no
/// control characters. What is left the shell only splits into words and
/// globs, which the same shell then does exactly as it did for the
/// TypeScript.
fn shell_safe(command: &str) -> bool {
    !command.chars().any(|c| matches!(c, '$' | '`' | ';' | '&' | '|' | '<' | '>' | '(' | ')' | '\\' | '\'' | '"') || c.is_control())
}

/// A message the shell takes literally inside `-m"…"` once the TypeScript
/// has put a backslash before every `"`: no `$`, backquote or backslash;
/// and no control character other than a line feed or a tab (a carriage
/// return or an escape would change what chalk and the terminal make of
/// the echo).
fn message_safe(message: &str) -> bool {
    message.len() <= MAX_MESSAGE && !message.chars().any(|c| matches!(c, '$' | '`' | '\\') || (c.is_control() && c != '\n' && c != '\t'))
}

/// `packageUp({ cwd })`: the directory of the nearest package.json file,
/// from the current directory up to the root.
fn package_dir(cwd: &Path) -> Option<&Path> {
    cwd.ancestors().find(|dir| std::fs::metadata(dir.join("package.json")).is_ok_and(|m| m.is_file()))
}

/// Where git keeps what the binary must look at before it copies the index.
struct Repo {
    index: PathBuf,
    top: PathBuf,
}

impl Repo {
    fn probe(cwd: &Path) -> Result<Repo, HandBack> {
        let out = std::process::Command::new("git")
            .args(["rev-parse", "--git-path", "index", "--git-path", "hooks/post-index-change", "--git-dir", "--show-toplevel"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|_| HandBack)?;
        if !out.status.success() {
            return Err(HandBack);
        }
        let text = String::from_utf8(out.stdout).map_err(|_| HandBack)?;
        let paths: Vec<PathBuf> = text.lines().map(|line| cwd.join(line)).collect();
        let [index, hook, git_dir, top] = paths.as_slice() else {
            return Err(HandBack);
        };
        let present = |path: &Path| !matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        // Another git at work (the TypeScript's `git add` would fail), a hook
        // that git runs whenever an index is written (it would run for the
        // copy too), a split index (whose shared part the copy would share).
        let mut lock = index.clone().into_os_string();
        lock.push(".lock");
        if present(Path::new(&lock)) || present(hook) {
            return Err(HandBack);
        }
        for dir in [git_dir.as_path(), index.parent().unwrap_or(git_dir)] {
            let entries = std::fs::read_dir(dir).map_err(|_| HandBack)?;
            for entry in entries {
                let entry = entry.map_err(|_| HandBack)?;
                if entry.file_name().to_string_lossy().starts_with("sharedindex.") {
                    return Err(HandBack);
                }
            }
        }
        Ok(Repo { index: index.clone(), top: top.clone() })
    }
}

/// A directory of the binary's own with a copy of the index, removed when
/// this is dropped (also when the command panics).
struct Copy {
    dir: PathBuf,
    index: OsString,
}

impl Copy {
    fn of(index: &Path, cwd: &Path) -> Result<Copy, HandBack> {
        let base = cwd.join(std::env::temp_dir());
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = base.join(format!("snocommit-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).map_err(|_| HandBack)?;
        let copy = Copy { index: dir.join("index").into_os_string(), dir };
        match std::fs::metadata(index) {
            Ok(meta) if meta.is_file() => {
                std::fs::copy(index, &copy.index).map_err(|_| HandBack)?;
                // git takes an entry changed in the second the index was
                // written for one to look at again: the copy keeps the time.
                let modified = meta.modified().map_err(|_| HandBack)?;
                let file = std::fs::File::options().write(true).open(&copy.index).map_err(|_| HandBack)?;
                file.set_times(std::fs::FileTimes::new().set_modified(modified)).map_err(|_| HandBack)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(HandBack),
        }
        Ok(copy)
    }

    /// Whether a command's output names the copy's directory: by its whole
    /// path (an error about the copy), or by its own name, as a status or a
    /// diff does where the temp directory lies inside the work tree, and the
    /// copy would be among the changes.
    fn mentioned_in(&self, out: &sh::Output) -> bool {
        let name = self.dir.file_name().map(|n| n.as_encoded_bytes()).unwrap_or_default();
        let has = |bytes: &[u8]| !name.is_empty() && bytes.windows(name.len()).any(|w| w == name);
        has(&out.stdout) || has(&out.stderr)
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn small(out: &sh::Output) -> Result<(), HandBack> {
    if out.code.is_none() || sh::decoded_len(&out.stdout) > MAX_OUTPUT || sh::decoded_len(&out.stderr) > MAX_OUTPUT {
        return Err(HandBack);
    }
    Ok(())
}

/// A command that must have gone as it does when nothing is wrong: exit
/// code 0, nothing on stderr, stdout in UTF-8 (node decodes it, replacing
/// what is not).
fn clean(out: &sh::Output) -> Result<&str, HandBack> {
    small(out)?;
    if out.code != Some(0) || !out.stderr.is_empty() {
        return Err(HandBack);
    }
    std::str::from_utf8(&out.stdout).map_err(|_| HandBack)
}

/// `getRecentCommits()` from the log's stdout (trimmed at the end).
fn recent_commits(stdout: &str) -> String {
    if stdout.is_empty() || js::trim(stdout).is_empty() {
        return "(no recent commits found)".to_string();
    }
    stdout
        .split('\n')
        .map(|line| {
            let mut parts = line.split('\t');
            let mut next = || parts.next().unwrap_or("undefined").to_string();
            let (hash, author, date) = (next(), next(), next());
            let msg: Vec<&str> = parts.collect();
            format!("- {hash} | {author} | {date} | {}", msg.join("\t"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One diff as the prompt carries it: cut to its first and last 1,000 code
/// units when it is longer than 2,000. A cut through a character of two
/// code units goes back.
fn truncated(diff: &str) -> Result<String, HandBack> {
    if js::utf16_len(diff) <= 2000 {
        return Ok(diff.to_string());
    }
    let head = js::head_units(diff, 1000).ok_or(HandBack)?;
    let tail = js::tail_units(diff, 1000).ok_or(HandBack)?;
    Ok(format!("{head}\n\n... (diff truncated) ...\n\n{tail}"))
}

/// The system message of src/index.tsx's generateCommitMessage.
fn prompt(cwd: &str, kind: &str, scope: &str, desc: &str, recent: &str, diffs: &[(String, String)]) -> String {
    let scope_string = if !scope.is_empty() && scope != "." { format!("({scope})") } else { String::new() };
    let files: Vec<&str> = diffs.iter().map(|(file, _)| file.as_str()).collect();
    let diff: Vec<String> = diffs.iter().map(|(file, diff)| format!("--- a/{file}\n+++ b/{file}\n{diff}")).collect();
    format!(
        "\nAssist me in generating a conventional commit message based on the following information.\n\n\
         cwd: {cwd}\nSuggested Commit Type: {kind}\nSuggested Scope: {}\nShort Description Provided by user: {desc}\n\n\
         Recent Commits:\n{recent}\n\nModified Files:\n{}\n\nDetailed Git Diff:\n```diff\n{}\n```\n\n\
         Commit Message Format:\n<type><scope>: <title>\n\n<body>\n[BREAKING CHANGE: <description of breaking change>s]\n  ",
        if scope_string.is_empty() { "none" } else { &scope_string },
        files.join("\n"),
        diff.join("\n"),
    )
}

/// Everything the binary found out, and will now do.
pub struct Plan {
    colour: bool,
    add: Option<String>,
    status: sh::Output,
    commit: Option<Commit>,
}

struct Commit {
    diffs: Vec<(String, sh::Output)>,
    key: OsString,
    log: sh::Output,
    message: String,
}

/// Works out the whole invocation, or hands it back. Nothing is printed,
/// and nothing outside the binary's copy of the index is changed.
pub fn prepare(command: &str, args: &[String], script: Option<&Path>) -> Result<Plan, HandBack> {
    // The shell is /bin/sh here; on Windows exec's is cmd.exe, whose rules
    // are not ported.
    if !cfg!(unix) {
        return Err(HandBack);
    }
    let kind = TYPES.iter().copied().find(|t| *t == command).ok_or(HandBack)?;
    let cwd = std::env::current_dir().map_err(|_| HandBack)?;
    let cwd_text = cwd.to_str().ok_or(HandBack)?.to_string();
    // A package root that holds `cli-` or `bin/` before the file's own name
    // makes the TypeScript take the first argument for the type.
    if !script.is_some_and(|script| script_says(script, &cwd_text, kind)) {
        return Err(HandBack);
    }
    // Without a description the TypeScript throws.
    let (scope, words) = args.split_first().ok_or(HandBack)?;
    let desc = words.join(" ");
    if desc.is_empty() || args.iter().any(|arg| color::FLAGS.contains(&arg.as_str())) {
        return Err(HandBack);
    }
    if !gate::environment_is_plain() {
        return Err(HandBack);
    }
    let home = ["HOME", "USERPROFILE"].iter().find_map(|name| std::env::var_os(name).filter(|v| !v.is_empty())).map(PathBuf::from);
    if !gate::runtime_is_node() && !gate::nothing_for_bun(&cwd, home.as_deref()) {
        return Err(HandBack);
    }
    let add = match scope.as_str() {
        "-" => None,
        "." => Some("git add .".to_string()),
        // DIE`No package.json found for "@" scope.` when there is none.
        "@" => Some(format!("git add {}", package_dir(&cwd).ok_or(HandBack)?.to_str().ok_or(HandBack)?)),
        path => {
            // An option to git add (`-A`, `--help`) is not a path.
            if path.is_empty() || path.split(' ').any(|word| word.starts_with('-')) {
                return Err(HandBack);
            }
            Some(format!("git add {path}"))
        }
    };
    if add.as_deref().is_some_and(|add| !shell_safe(add)) {
        return Err(HandBack);
    }
    let colour = color::stdout_has_colour(|name| std::env::var(name).ok(), std::io::IsTerminal::is_terminal(&std::io::stdout()));

    let repo = Repo::probe(&cwd)?;
    let copy = Copy::of(&repo.index, &cwd)?;
    // A temp directory inside the work tree: the copy would be among the
    // changes the copy's status shows.
    let real = |path: &Path| std::fs::canonicalize(path).map_err(|_| HandBack);
    if real(&copy.dir)?.starts_with(real(&repo.top)?) {
        return Err(HandBack);
    }
    let on_copy = [("GIT_INDEX_FILE", Some(&copy.index))];
    if let Some(add) = &add {
        let out = sh::run(add, &on_copy, false);
        small(&out)?;
        if copy.mentioned_in(&out) {
            return Err(HandBack);
        }
    }
    let status = sh::run(STATUS, &on_copy, false);
    if copy.mentioned_in(&status) {
        return Err(HandBack);
    }
    let lines: Vec<String> = js::trim_end(clean(&status)?).split('\n').map(js::trim).filter(|line| !line.is_empty()).map(str::to_string).collect();
    if lines.is_empty() {
        return Ok(Plan { colour, add, status, commit: None });
    }
    let mut diffs = Vec::new();
    let mut shown = Vec::new();
    for line in &lines {
        let command = format!("git diff --cached -- {line}");
        if !shell_safe(&command) {
            return Err(HandBack);
        }
        let out = sh::run(&command, &on_copy, false);
        if copy.mentioned_in(&out) {
            return Err(HandBack);
        }
        shown.push((line.clone(), truncated(js::trim_end(clean(&out)?))?));
        diffs.push((command, out));
    }
    drop(copy);

    let key = openai::api_key()?;
    let url = openai::endpoint()?;
    let key = OsString::from(key);
    let log = sh::run(LOG, &[("OPENAI_API_KEY", Some(&key))], false);
    small(&log)?;
    let recent = recent_commits(js::trim_end(std::str::from_utf8(&log.stdout).map_err(|_| HandBack)?));
    let body = openai::request_body(&prompt(&cwd_text, kind, scope, &desc, &recent, &shown));
    let answer = openai::ask(&url, key.to_str().ok_or(HandBack)?, &body)?;
    let message = openai::message(&openai::proposal(&answer)?)?;
    if !message_safe(&message) {
        return Err(HandBack);
    }
    Ok(Plan { colour, add, status, commit: Some(Commit { diffs, key, log, message }) })
}

impl Plan {
    /// Does what the TypeScript does, in its order, with its output, and
    /// returns the exit code.
    pub fn carry_out(self) -> i32 {
        let echo = |command: &str| {
            let line = format!("> {command}");
            sh::print(&format!("{}\n", if self.colour { color::blue(&line) } else { line }));
        };
        if let Some(add) = &self.add {
            echo(add);
            sh::run(add, &[], true);
        }
        echo(STATUS);
        if sh::run(STATUS, &[], true) != self.status {
            return changed();
        }
        let Some(commit) = &self.commit else {
            sh::print("No files staged to commit.\n");
            return 0;
        };
        for (command, before) in &commit.diffs {
            echo(command);
            if sh::run(command, &[], true) != *before {
                return changed();
            }
        }
        // From here on the TypeScript has put the key into process.env.
        let key = [("OPENAI_API_KEY", Some(&commit.key))];
        echo(LOG);
        if sh::run(LOG, &key, true) != commit.log {
            return changed();
        }
        sh::print(&format!("Generating commit message with {}...\n", openai::MODEL));
        sh::print(&format!("Committing with message:\n {}\n", commit.message));
        let commit_line = format!("git commit -m\"{}\"", commit.message.replace('"', "\\\""));
        echo(&commit_line);
        sh::run(&commit_line, &key, true);
        echo(PULL_PUSH);
        sh::run(PULL_PUSH, &key, true);
        sh::print("Commit pushed successfully.\n");
        0
    }
}

/// The repository came out differently from what the binary had seen on its
/// copy: something else changed it meanwhile. The message was written for
/// what the copy showed, so nothing is committed.
fn changed() -> i32 {
    sh::eprint(
        "snocommit: the repository changed while the commit message was being written, so nothing was committed \
         (what is staged stays staged). Run the command again; with SNOCOMMIT_NO_NATIVE=1 the JavaScript does it.\n",
    );
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_comes_from_the_file_name() {
        assert_eq!(type_from_script("/usr/lib/node_modules/snocommit/dist/cli-feat.js"), Some("feat"));
        assert_eq!(type_from_script("/x/dist/cli-chore.mjs"), Some("chore"));
        assert_eq!(type_from_script("/x/bin/fix"), Some("fix"));
        assert_eq!(type_from_script("/x/node_modules/.bin/styles"), Some("styles"));
        assert_eq!(type_from_script("/home/robin/.bun/dist/cli-feat.js"), None);
        assert_eq!(type_from_script("/x/cli-tools/dist/cli-feat.js"), None);
        assert_eq!(type_from_script("/x/dist/cli-feat.jsx"), None);
        assert_eq!(type_from_script("/x/dist/cli-feat"), Some("feat"));
        assert_eq!(type_from_script("/x/dist/cli-snocommit.js"), None);
        assert_eq!(type_from_script("/x/dist/cli.js"), None);
        assert_eq!(type_from_script("/x\n/dist/cli-feat.js"), None);
    }

    #[test]
    fn paths_resolve_as_node_resolves_them() {
        assert_eq!(resolve("/c", "/a/./b//c/../d/"), "/a/b/d");
        assert_eq!(resolve("/c/w", "x/../y"), "/c/w/y");
        assert_eq!(resolve("/", "/../a"), "/a");
    }

    #[test]
    fn shells() {
        assert!(shell_safe("git add src/*.ts ?? #x ~ a=b @ %"));
        for bad in ["git add $(x)", "git add `x`", "git add a;b", "git add a&", "git add a|b", "git add a>b", "git add a<b", "git add (a)", "git add 'a'", "git add \"a\"", "git add a\\b", "git add a\tb", "git add a\nb"] {
            assert!(!shell_safe(bad), "{bad}");
        }
        assert!(message_safe("feat(x)!: a \"b\" 'c' !d\n\n\tbody é"));
        for bad in ["a $b", "a `b`", "a \\ b", "a\rb", "a\u{1b}[0m", "a\0"] {
            assert!(!message_safe(bad), "{bad:?}");
        }
    }

    #[test]
    fn recent_commits_as_the_typescript_writes_them() {
        assert_eq!(recent_commits(""), "(no recent commits found)");
        assert_eq!(recent_commits("abc\tA B\t2020-01-01\tfeat: x\ty"), "- abc | A B | 2020-01-01 | feat: x\ty");
        assert_eq!(recent_commits("abc"), "- abc | undefined | undefined | ");
        assert_eq!(recent_commits("a\tb\tc\td\ne\tf\tg\th"), "- a | b | c | d\n- e | f | g | h");
    }

    #[test]
    fn diffs_are_cut_by_code_units() {
        let short = "x".repeat(2000);
        assert_eq!(truncated(&short).unwrap(), short);
        let long = format!("{}{}", "a".repeat(1000), "b".repeat(1001));
        assert_eq!(truncated(&long).unwrap(), format!("{}\n\n... (diff truncated) ...\n\n{}", "a".repeat(1000), "b".repeat(1000)));
        let split = format!("{}😀{}", "a".repeat(999), "b".repeat(1500));
        assert!(truncated(&split).is_err());
    }
}
