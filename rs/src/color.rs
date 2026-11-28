//! Whether the echo of a git command is blue: chalk's `blue`, with the level
//! supports-color gives stdout (both bundled into dist/cli.js through snorun).
//!
//! Every level above 0 gives the same 16-colour blue, so only "any colour or
//! none" matters here. What supports-color reads that the port does not
//! spell out goes back to the JavaScript before this is asked (see
//! `gate::environment`): FORCE_COLOR, TF_BUILD and TEAMCITY_VERSION, and a
//! colour flag among the arguments (`--color`, `--no-color`, …), which
//! supports-color looks for in the command's own arguments.

/// The flags supports-color looks for in `process.argv`. One of them among
/// the arguments sends the invocation back.
pub const FLAGS: [&str; 12] = [
    "--no-color",
    "--no-colors",
    "--color=false",
    "--color=never",
    "--color",
    "--colors",
    "--color=true",
    "--color=always",
    "--color=16m",
    "--color=full",
    "--color=truecolor",
    "--color=256",
];

/// `supportsColor.stdout` is not false, for a stdout that is (`tty`) or is
/// not a terminal, with FORCE_COLOR, TF_BUILD and TEAMCITY_VERSION unset and
/// no colour flag among the arguments (and not on Windows, where this port
/// hands everything back).
pub fn stdout_has_colour(var: impl Fn(&str) -> Option<String>, tty: bool) -> bool {
    let set = |name: &str| var(name).is_some();
    // `haveStream && !streamIsTTY && forceColor === undefined`
    if !tty {
        return false;
    }
    let term = var("TERM");
    if term.as_deref() == Some("dumb") {
        return false;
    }
    if set("CI") {
        return ["GITHUB_ACTIONS", "GITEA_ACTIONS", "CIRCLECI", "TRAVIS", "APPVEYOR", "GITLAB_CI", "BUILDKITE", "DRONE"]
            .iter()
            .any(|name| set(name))
            || var("CI_NAME").as_deref() == Some("codeship");
    }
    if var("COLORTERM").as_deref() == Some("truecolor") || term.as_deref() == Some("xterm-kitty") {
        return true;
    }
    if matches!(var("TERM_PROGRAM").as_deref(), Some("iTerm.app" | "Apple_Terminal")) {
        return true;
    }
    // `/-256(color)?$/i` and `/^screen|^xterm|^vt100|^vt220|^rxvt|color|ansi|cygwin|linux/i`,
    // tried on `String(env.TERM)`, which is "undefined" when TERM is unset.
    let term = term.unwrap_or_else(|| "undefined".to_string()).to_ascii_lowercase();
    if term.ends_with("-256") || term.ends_with("-256color") {
        return true;
    }
    if ["screen", "xterm", "vt100", "vt220", "rxvt"].iter().any(|start| term.starts_with(start))
        || ["color", "ansi", "cygwin", "linux"].iter().any(|part| term.contains(part))
    {
        return true;
    }
    set("COLORTERM")
}

/// `chalk.blue(text)` at a level above 0: the colour opened at the start,
/// closed at the end, and closed and opened again around every line feed
/// (the text has no ESC and no CR; the caller hands those back).
pub fn blue(text: &str) -> String {
    format!("\x1b[34m{}\x1b[39m", text.replace('\n', "\x1b[39m\n\x1b[34m"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &[(&str, &str)], tty: bool) -> bool {
        let map: std::collections::HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        stdout_has_colour(|name| map.get(name).cloned(), tty)
    }

    #[test]
    fn levels() {
        assert!(!with(&[("TERM", "xterm-256color")], false));
        assert!(with(&[("TERM", "xterm-256color")], true));
        assert!(!with(&[("TERM", "dumb"), ("COLORTERM", "truecolor")], true));
        assert!(!with(&[], true));
        assert!(with(&[("COLORTERM", "1")], true));
        assert!(with(&[("TERM", "Linux")], true));
        assert!(with(&[("TERM", "foo-256")], true));
        assert!(!with(&[("TERM", "foo-2566")], true));
        assert!(!with(&[("CI", "1"), ("TERM", "xterm")], true));
        assert!(with(&[("CI", ""), ("GITHUB_ACTIONS", "")], true));
        assert!(with(&[("CI", "1"), ("CI_NAME", "codeship")], true));
        assert!(with(&[("TERM_PROGRAM", "Apple_Terminal"), ("TERM", "dumb2")], true));
        assert!(!with(&[("TERM_PROGRAM", "vscode")], true));
        assert!(with(&[("TERM", "mycolorterm")], true));
    }

    #[test]
    fn blue_closes_around_line_feeds() {
        assert_eq!(blue("> a"), "\x1b[34m> a\x1b[39m");
        assert_eq!(blue("a\n\nb"), "\x1b[34ma\x1b[39m\n\x1b[34m\x1b[39m\n\x1b[34mb\x1b[39m");
    }
}
