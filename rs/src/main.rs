//! snocommit — Rust entry point.
//!
//! Installed from npm, this binary is the command users run. A command runs
//! here once it has been ported and a parity test shows it behaves like the
//! TypeScript; until then it is handed back to the JavaScript the package
//! still ships, so nothing changes for users.
//!
//! The six commands (chore, docs, feat, fix, refactor, styles) are one
//! program, which takes its commit type from the name it was started as.
//! Ported: their whole pipeline on Linux and macOS (rs/src/commit.rs), for
//! every invocation and state of the machine it reproduces exactly; the rest
//! goes back. Windows goes back entirely.

mod color;
mod commit;
mod gate;
mod js;
mod js_fallback;
mod openai;
mod sh;

/// The command this binary was started as.
fn command() -> String {
    std::env::args_os()
        .next()
        .map(std::path::PathBuf::from)
        .and_then(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// The JavaScript entry of each command, relative to the package root.
fn js_entry(command: &str) -> &'static str {
    match command {
        "docs" => "dist/cli-docs.js",
        "feat" => "dist/cli-feat.js",
        "fix" => "dist/cli-fix.js",
        "refactor" => "dist/cli-refactor.js",
        "snocommit" => "dist/cli-snocommit.js",
        "styles" => "dist/cli-styles.js",
        _ => "dist/cli-chore.js", // chore
    }
}

/// `SNOCOMMIT_NO_NATIVE=1` hands every command back to the JavaScript: the way
/// out for a user if a ported command misbehaves.
fn native() -> bool {
    std::env::var_os("SNOCOMMIT_NO_NATIVE").is_none()
}

fn main() {
    let command = command();
    // Arguments that are not valid UTF-8 are for the JavaScript to deal with.
    let args: Option<Vec<String>> = std::env::args_os().skip(1).map(|arg| arg.into_string().ok()).collect();
    if let Some(args) = args.filter(|_| native()) {
        // Everything the binary answers itself belongs here, so that
        // `SNOCOMMIT_NO_NATIVE=1` switches it off. `commit::prepare` works out
        // the whole invocation without printing or changing anything, or
        // declines; should it panic, the invocation is handed back just the
        // same. Only then is the plan carried out, and there is no way back.
        if matches!(command.as_str(), "chore" | "docs" | "feat" | "fix" | "refactor" | "styles") {
            let script = js_fallback::package_root().map(|root| root.join(js_entry(&command)));
            if let Some(plan) = js_fallback::attempt(|| commit::prepare(&command, &args, script.as_deref())) {
                std::process::exit(plan.carry_out());
            }
        }
    }
    js_fallback::run(js_entry(&command))
}
