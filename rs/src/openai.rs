//! The commit message: where the API key comes from, the one chat completion
//! the TypeScript asks for (z-chat-completion over the openai SDK), and what
//! it makes of the answer.
//!
//! Every way this can go wrong in the TypeScript ends the program as an
//! unhandled rejection in the runtime's own words (or, for a missing key,
//! asks on the terminal), so here every such way is `Err(HandBack)`, and the
//! caller has staged, printed and committed nothing yet.

use crate::js;
use crate::js_fallback::HandBack;
use serde_json::Value;
use std::path::Path;

/// The six commit types, in the TypeScript's order (src/commitTypes.ts).
pub const TYPES: [&str; 6] = ["fix", "styles", "feat", "docs", "chore", "refactor"];

/// The model the TypeScript names.
pub const MODEL: &str = "gpt-4o";

/// `validApiKey`: a key that starts with `sk-` and is longer than 20 code
/// units, trimmed. Keys with anything but printable ASCII are handed back:
/// their length, their trimming and their way into a header are the
/// runtime's to decide.
fn valid_key(key: &str) -> Result<Option<String>, HandBack> {
    if key.is_empty() {
        return Ok(None);
    }
    if !key.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return Err(HandBack);
    }
    if key.starts_with("sk-") && key.len() > 20 {
        return Ok(Some(key.trim_matches(' ').to_string()));
    }
    Ok(None)
}

/// `getOpenAIApiKey`, as far as it goes without asking: `.snocommitrc.json`
/// in the home directory (`$HOME`, else `$USERPROFILE`), then
/// `$OPENAI_API_KEY`. No key means the TypeScript would ask on the terminal
/// and save the answer: that is handed back.
pub fn api_key() -> Result<String, HandBack> {
    let home = ["HOME", "USERPROFILE"].iter().find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()));
    if let Some(home) = home {
        // path.join tidies `.` and `..` away before the file is read, which
        // is not the same file where a link is in the way.
        if !home.starts_with('/') || home.split('/').any(|part| part == "." || part == "..") {
            return Err(HandBack);
        }
        if let Some(key) = key_from_rc(&Path::new(&home).join(".snocommitrc.json"))? {
            return Ok(key);
        }
    }
    if let Ok(env) = std::env::var("OPENAI_API_KEY") {
        if let Some(key) = valid_key(&env)? {
            return Ok(key);
        }
    }
    Err(HandBack)
}

/// The rc file's key, when it has one the TypeScript takes. A file that is
/// not there, or that the TypeScript would read and drop (`catch {}`), gives
/// none; one that only JavaScript reads with certainty is handed back.
fn key_from_rc(path: &Path) -> Result<Option<String>, HandBack> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(HandBack),
    };
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err(HandBack);
    }
    let bytes = std::fs::read(path).map_err(|_| HandBack)?;
    // node decodes it with replacement characters; JSON.parse sees those.
    let text = String::from_utf8(bytes).map_err(|_| HandBack)?;
    key_from_rc_text(&text)
}

fn key_from_rc_text(text: &str) -> Result<Option<String>, HandBack> {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        // JSON.parse refuses it too, unless serde refuses what JavaScript
        // takes (a number past a double, a lone surrogate, deep nesting).
        Err(_) => return if json_parse_also_fails(text) { Ok(None) } else { Err(HandBack) },
    };
    // zSnocommitRC: an object whose OPENAI_API_KEY is a string matching /^sk-/.
    let Some(key) = value.as_object().and_then(|o| o.get("OPENAI_API_KEY")).and_then(Value::as_str) else {
        return Ok(None);
    };
    if !key.starts_with("sk-") {
        return Ok(None);
    }
    valid_key(key)
}

/// Text that no JSON parser takes: not one of the things serde and
/// JavaScript disagree about. Conservative: when in doubt, false.
fn json_parse_also_fails(text: &str) -> bool {
    let bytes = text.as_bytes();
    let exponent = bytes.windows(2).any(|w| w[0].is_ascii_digit() && matches!(w[1], b'e' | b'E'));
    let longest_digits = text.split(|c: char| !c.is_ascii_digit()).map(str::len).max().unwrap_or(0);
    !exponent && !text.contains('\\') && longest_digits < 300 && text.len() < 64 * 1024 && bytes.iter().filter(|b| matches!(b, b'[' | b'{')).count() < 100
}

/// Where the request goes: `${baseURL}/chat/completions`, with the base from
/// OPENAI_BASE_URL (trimmed; empty is the default) as the SDK takes it. The
/// binary goes only to the default address, over HTTPS, and to
/// `http://127.0.0.1:<port>[/path]`, the form a stand-in or a model server
/// on the same machine has; any other base is handed back.
pub fn endpoint() -> Result<String, HandBack> {
    let base = match std::env::var("OPENAI_BASE_URL") {
        Ok(base) if !base.is_empty() => base,
        Ok(_) => return Ok("https://api.openai.com/v1/chat/completions".to_string()),
        Err(std::env::VarError::NotPresent) => return Ok("https://api.openai.com/v1/chat/completions".to_string()),
        Err(_) => return Err(HandBack),
    };
    let rest = base.strip_prefix("http://127.0.0.1:").ok_or(HandBack)?;
    let (port, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    // new URL() writes the port as a number and leaves out 80.
    if port.is_empty() || port.len() > 5 || port.starts_with('0') || !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err(HandBack);
    }
    let number: u32 = port.parse().map_err(|_| HandBack)?;
    if number > 65535 || number == 80 {
        return Err(HandBack);
    }
    let path = path.strip_suffix('/').unwrap_or(path);
    // Plain segments only: new URL() would rewrite dots, escapes and more.
    let plain = |segment: &str| {
        !segment.is_empty() && segment != "." && segment != ".." && segment.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    };
    if !path.is_empty() && !path[1..].split('/').all(plain) {
        return Err(HandBack);
    }
    Ok(format!("http://127.0.0.1:{port}{path}/chat/completions"))
}

/// The JSON schema z.toJSONSchema writes for the TypeScript's zod object, as
/// the SDK sends it inside `response_format`.
const RESPONSE_FORMAT: &str = r#"{"type":"json_schema","json_schema":{"name":"schema","strict":true,"schema":{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"type":{"type":"string","enum":["fix","styles","feat","docs","chore","refactor"]},"scope":{"type":"string"},"title":{"type":"string","minLength":1,"maxLength":72},"body":{"type":"string","minLength":1},"is_BREAKING_CHANGE":{"type":"boolean"}},"required":["type","scope","title","body","is_BREAKING_CHANGE"],"additionalProperties":false}}}"#;

/// The request body, byte for byte as the SDK writes it: JSON.stringify of
/// `{ response_format, messages, model, ...options }`.
pub fn request_body(prompt: &str) -> String {
    format!(
        r#"{{"response_format":{RESPONSE_FORMAT},"messages":[{{"role":"system","content":{}}}],"model":{}}}"#,
        js::json_string(prompt),
        js::json_string(MODEL)
    )
}

/// What the model proposed, checked as the zod schema checks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub kind: String,
    pub scope: String,
    pub title: String,
    pub body: String,
    pub breaking: bool,
}

/// `response?.choices?.[0]?.message?.content`, parsed and checked. Anything
/// the TypeScript would throw at, or that only JavaScript reads with
/// certainty, is handed back.
pub fn proposal(response: &str) -> Result<Proposal, HandBack> {
    let response: Value = serde_json::from_str(response).map_err(|_| HandBack)?;
    let content = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .filter(|content| !content.is_empty())
        .ok_or(HandBack)?;
    let generated: Value = serde_json::from_str(content).map_err(|_| HandBack)?;
    let generated = generated.as_object().ok_or(HandBack)?;
    let string = |name: &str| generated.get(name).and_then(Value::as_str).map(str::to_string).ok_or(HandBack);
    let kind = string("type")?;
    if !TYPES.contains(&kind.as_str()) {
        return Err(HandBack);
    }
    let scope = string("scope")?;
    let title = string("title")?;
    if !(1..=72).contains(&js::utf16_len(&title)) {
        return Err(HandBack);
    }
    let body = string("body")?;
    if body.is_empty() {
        return Err(HandBack);
    }
    let breaking = generated.get("is_BREAKING_CHANGE").and_then(Value::as_bool).ok_or(HandBack)?;
    Ok(Proposal { kind, scope, title, body, breaking })
}

/// `generated.scope.replace(/^\(?(.+?)\)?$/, "($1)")`: one pair of
/// parentheses around a scope that is not empty. A scope with a line
/// terminator in it (which `.` does not match) is handed back.
fn scope_part(scope: &str) -> Result<String, HandBack> {
    if scope.contains(js::is_line_terminator) {
        return Err(HandBack);
    }
    if scope.is_empty() {
        return Ok(String::new());
    }
    // `\(?` takes a leading "(" unless nothing would be left for `(.+?)`.
    let inner = match scope.strip_prefix('(') {
        Some(rest) if !rest.is_empty() => rest,
        _ => scope,
    };
    // The shortest `(.+?)` after which only `\)?` and the end remain.
    let inner = match inner.strip_suffix(')') {
        Some(head) if !head.is_empty() => head,
        _ => inner,
    };
    Ok(format!("({inner})"))
}

/// The message the TypeScript commits: `<type><(scope)><!>: <title>`, a
/// blank line, and the body.
pub fn message(p: &Proposal) -> Result<String, HandBack> {
    Ok(format!("{}{}{}: {}\n\n{}", p.kind, scope_part(&p.scope)?, if p.breaking { "!" } else { "" }, p.title, p.body))
}

/// Sends the request once. `Ok` only for a 200 with a JSON body that is
/// valid UTF-8; everything else (no connection, another status, a redirect,
/// another kind of body) is handed back, and the JavaScript asks again,
/// with the SDK's own retries and errors.
#[cfg(feature = "net")]
pub fn ask(url: &str, key: &str, body: &str) -> Result<String, HandBack> {
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .proxy(None)
        .user_agent(concat!("snocommit/", env!("CARGO_PKG_VERSION")))
        // The SDK's own timeout.
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .build()
        .new_agent();
    let mut response = agent
        .post(url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {key}"))
        .send(body.as_bytes())
        .map_err(|_| HandBack)?;
    if response.status().as_u16() != 200 {
        return Err(HandBack);
    }
    // The SDK parses JSON when the media type includes application/json (or
    // ends with +json); exactly that one, once, is taken here.
    let types: Vec<_> = response.headers().get_all("content-type").iter().collect();
    let json = matches!(types.as_slice(), [one] if one.to_str().is_ok_and(|v| js::trim(v.split(';').next().unwrap_or("")) == "application/json"));
    if !json {
        return Err(HandBack);
    }
    let bytes = response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec().map_err(|_| HandBack)?;
    let text = String::from_utf8(bytes).map_err(|_| HandBack)?;
    // fetch's .json() drops a byte order mark; serde does not take one.
    if text.starts_with('\u{feff}') {
        return Err(HandBack);
    }
    Ok(text)
}

#[cfg(not(feature = "net"))]
pub fn ask(_url: &str, _key: &str, _body: &str) -> Result<String, HandBack> {
    Err(HandBack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_get_one_pair_of_parentheses() {
        let part = |s: &str| scope_part(s).unwrap();
        assert_eq!(part(""), "");
        assert_eq!(part("ui"), "(ui)");
        assert_eq!(part("(ui)"), "(ui)");
        assert_eq!(part("ui)"), "(ui)");
        assert_eq!(part("(ui"), "(ui)");
        assert_eq!(part("("), "(()");
        assert_eq!(part(")"), "())");
        assert_eq!(part("()"), "())");
        assert_eq!(part("(("), "(()");
        assert_eq!(part("))"), "())");
        assert_eq!(part("((x))"), "((x))");
        assert_eq!(part("a)b"), "(a)b)");
        assert!(scope_part("a\nb").is_err());
        assert!(scope_part("a\u{2028}").is_err());
    }

    #[test]
    fn keys() {
        assert_eq!(valid_key("sk-0123456789abcdefgh").unwrap(), Some("sk-0123456789abcdefgh".to_string()));
        assert_eq!(valid_key("sk-0123456789abcdefg").unwrap(), None);
        assert_eq!(valid_key("pk-0123456789abcdefghij").unwrap(), None);
        assert_eq!(valid_key("sk-0123456789abcdefgh  ").unwrap(), Some("sk-0123456789abcdefgh".to_string()));
        assert_eq!(valid_key("sk-0123456789abcdef  ").unwrap(), Some("sk-0123456789abcdef".to_string()));
        assert!(valid_key("sk-0123456789abcdefgh\t").is_err());
        assert_eq!(key_from_rc_text(r#"{"OPENAI_API_KEY":"sk-0123456789abcdefghij"}"#).unwrap(), Some("sk-0123456789abcdefghij".to_string()));
        assert_eq!(key_from_rc_text(r#"{"OPENAI_API_KEY":" sk-0123456789abcdefghij"}"#).unwrap(), None);
        assert_eq!(key_from_rc_text(r#"{"OPENAI_API_KEY":5}"#).unwrap(), None);
        assert_eq!(key_from_rc_text("[1]").unwrap(), None);
        assert_eq!(key_from_rc_text("{oops").unwrap(), None);
        assert_eq!(key_from_rc_text(r#"{"OPENAI_API_KEY":"sk-0123456789abcdefghij",}"#).unwrap(), None);
        assert!(key_from_rc_text(r#"{"OPENAI_API_KEY":"sk-0123456789abcdefghij","n":1E5,}"#).is_err());
        assert!(key_from_rc_text(r#"{"OPENAI_API_KEY":"\ud800",}"#).is_err());
        assert!(key_from_rc_text(r#"{"x":1e999,"OPENAI_API_KEY":"sk-0123456789abcdefghij"}"#).is_err());
    }

    #[test]
    fn endpoints() {
        let with = |value: &str| {
            std::env::set_var("OPENAI_BASE_URL", value);
            let out = endpoint();
            std::env::remove_var("OPENAI_BASE_URL");
            out
        };
        assert_eq!(with("http://127.0.0.1:4000/v1").unwrap(), "http://127.0.0.1:4000/v1/chat/completions");
        assert_eq!(with("http://127.0.0.1:4000/v1/").unwrap(), "http://127.0.0.1:4000/v1/chat/completions");
        assert_eq!(with("http://127.0.0.1:4000").unwrap(), "http://127.0.0.1:4000/chat/completions");
        assert_eq!(with("http://127.0.0.1:4000/").unwrap(), "http://127.0.0.1:4000/chat/completions");
        assert_eq!(with("").unwrap(), "https://api.openai.com/v1/chat/completions");
        for bad in ["http://127.0.0.1:80/v1", "http://127.0.0.1:04000", "http://localhost:4000", " http://127.0.0.1:4000", "http://127.0.0.1:4000/a/../b", "http://127.0.0.1:4000//v1", "https://example.com/v1", "http://127.0.0.1:"] {
            assert!(with(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn proposals() {
        let answer = |content: &str| format!(r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#, js::json_string(content));
        let p = proposal(&answer(r#"{"type":"feat","scope":"ui","title":"t","body":"b","is_BREAKING_CHANGE":true,"extra":1}"#)).unwrap();
        assert_eq!(message(&p).unwrap(), "feat(ui)!: t\n\nb");
        let title72 = "x".repeat(70) + "😀";
        assert!(proposal(&answer(&format!(r#"{{"type":"fix","scope":"","title":"{title72}","body":"b","is_BREAKING_CHANGE":false}}"#))).is_ok());
        let title73 = "x".repeat(71) + "😀";
        assert!(proposal(&answer(&format!(r#"{{"type":"fix","scope":"","title":"{title73}","body":"b","is_BREAKING_CHANGE":false}}"#))).is_err());
        assert!(proposal(&answer(r#"{"type":"perf","scope":"","title":"t","body":"b","is_BREAKING_CHANGE":false}"#)).is_err());
        assert!(proposal(&answer(r#"{"type":"fix","scope":"","title":"","body":"b","is_BREAKING_CHANGE":false}"#)).is_err());
        assert!(proposal(&answer(r#"{"type":"fix","scope":"","title":"t","body":"","is_BREAKING_CHANGE":false}"#)).is_err());
        assert!(proposal(&answer(r#"{"type":"fix","scope":"","title":"t","body":"b","is_BREAKING_CHANGE":"no"}"#)).is_err());
        assert!(proposal(&answer(r#"{"type":"fix","title":"t","body":"b","is_BREAKING_CHANGE":false}"#)).is_err());
        assert!(proposal(&answer("not json")).is_err());
        assert!(proposal(&answer("")).is_err());
        assert!(proposal(r#"{"choices":[]}"#).is_err());
    }

    #[test]
    fn the_body_starts_and_ends_as_the_sdk_writes_it() {
        let body = request_body("hi\n");
        assert!(body.starts_with(r#"{"response_format":{"type":"json_schema","#));
        assert!(body.ends_with(r#""messages":[{"role":"system","content":"hi\n"}],"model":"gpt-4o"}"#));
    }
}
