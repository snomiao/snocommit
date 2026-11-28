#!/usr/bin/env node
// Writes rs/parity.json, the parity cases for the six commands (Rust against
// TypeScript).
//
//   node rs/parity.gen.mjs            rewrite rs/parity.json
//   node rs/parity.gen.mjs --check    fail if rs/parity.json is out of date
//
// How to run them (from the repository root, after `bun install`, `bun run
// build` and `cargo build --release --manifest-path rs/Cargo.toml`):
// `sh rs/parity/proof.sh <rustify's templates/parity/parity.mjs>
// rs/target/release/snocommit` runs every proof there is on this system, as
// the CI workflow does on Linux, macOS and Windows. What it runs, for one
// directory <bin> holding the binary under the six commands' names:
//
//   node <parity.mjs> --cases rs/parity.json \
//     --ts "node rs/parity/with-mock.mjs ts" \
//     --rs "env SNOCOMMIT_PACKAGE_ROOT=$PWD node rs/parity/with-mock.mjs <bin>"
//   node <parity.mjs> --cases rs/parity.json --filter "[feat]" \
//     --rs "node rs/parity/with-mock.mjs <bin>" \
//     --handback SNOCOMMIT_PACKAGE_ROOT=dist/cli-feat.js
//
// and the same --handback check for each of the other five commands (each
// has its own JavaScript file). Every case names its command first in its
// name, `[feat] …`, for that.
//
// rs/parity/with-mock.mjs starts the command (see there) against a stand-in
// for the OpenAI API on 127.0.0.1, whose address it hands the command in
// OPENAI_BASE_URL, and writes down every request the stand-in got (one line
// each, the body as it came); most cases print that log afterwards. So a
// case compares what the two implementations asked the model for, byte for
// byte, and not only what they printed. A case runs in a git repository of its own, with a bare
// repository of its own under <ROOT> as the remote that is pulled from and
// pushed to, and fixed names and dates for every commit, so that hashes
// are the same on both sides.
//
// Nothing here reaches OpenAI: with-mock.mjs refuses any address but
// 127.0.0.1, and every key in here is made up.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const flags = new Set(process.argv.slice(2));
const cases = [];

// ── scenery ────────────────────────────────────────────────────────────────

const KEY = "sk-parity-env-0123456789abcdef";
const RC_KEY = "sk-parity-rc-0123456789abcdefg";

/** A repository with one commit, pushed to <ROOT>/remote.git, which it tracks. */
const REPO = [
  "git init -q -b main",
  "git init -q --bare -b main <ROOT>/remote.git",
  "git remote add origin <ROOT>/remote.git",
  "printf 'one\\n' > a.txt && git add a.txt && git commit -q -m 'chore: start' && git push -q -u origin main",
];
/** One line more in a.txt, and a new file b.txt. */
const CHANGES = ["printf 'two\\n' >> a.txt", "printf 'bee\\n' > b.txt"];

const LOG = "cat <ROOT>/openai.log";
/** What a case looks at afterwards: the commits, the working tree, the remote, the requests. */
const AFTER = [
  "git log --format='%h %an %ad %s%n%b' --date=iso HEAD",
  "git status --porcelain",
  "git -C <ROOT>/remote.git log --format='%h %s' main",
  LOG,
];
/** The same, for a case that the binary hands back after it has asked the model: the JavaScript asks again (and the SDK retries), so the requests are compared as a set. */
const AFTER_ASKED = [...AFTER.slice(0, 3), "sort -u <ROOT>/openai.log"];

/** A chat completion whose message is `fields` as JSON (the form the schema asks for). */
const answer = (fields) => ({ content: JSON.stringify(fields) });
const proposal = (over = {}) => ({ type: "feat", scope: "core", title: "add the second file", body: "Adds b.txt and a line to a.txt.", is_BREAKING_CHANGE: false, ...over });
const ok = (over = {}) => answer(proposal(over));

/**
 * One case. `o.cmd` (default feat) is the command, `o.args` what follows it;
 * `o.reply` what the stand-in answers (one reply or a list); `o.repo` false
 * for no repository; `o.setup` lines after the repository's; `o.after`
 * instead of AFTER; `o.env`; `o.handback` (text, or per system); `o.asked`
 * when the binary hands back after its request.
 */
function add(name, o = {}) {
  const cmd = o.cmd ?? "feat";
  const env = { ...(o.env ?? {}) };
  if (o.reply !== undefined) env.PARITY_OPENAI = JSON.stringify(o.reply);
  const testCase = {
    name: `[${cmd}] ${name}`,
    args: [cmd, ...(o.args ?? [".", "add the second file"])],
    ...(Object.keys(env).length ? { env } : {}),
    ...(o.files ? { files: o.files } : {}),
    setup: [...(o.repo === false ? [] : REPO), ...(o.setup ?? CHANGES)],
    after: o.after ?? (o.asked ? AFTER_ASKED : AFTER),
    ...(o.stdin !== undefined ? { stdin: o.stdin } : {}),
    ...(o.tty !== undefined ? { tty: o.tty } : {}),
    ...(o.capture ? { capture: o.capture } : {}),
    ...(o.timeout ? { timeout: o.timeout } : {}),
    ...(o.handback ? { handback: o.handback } : {}),
    ...(o.skip ? { skip: o.skip } : {}),
  };
  if (cases.some((c) => c.name === testCase.name)) throw new Error(`two cases named ${JSON.stringify(testCase.name)}`);
  if (o.asked && !o.handback) throw new Error(`${testCase.name}: "asked" is for cases handed back after the request`);
  cases.push(testCase);
}

const THROWS = "the TypeScript throws, and the runtime reports the unhandled rejection in its own words";
const ASKS = "without a key the TypeScript asks for one on the terminal and saves it";
const SHELL = "the shell would run, substitute or redirect part of this command line (the TypeScript lets it)";
const BUN = "a variable or file that bun or node acts on in ways the port does not follow";

// ── the pipeline ───────────────────────────────────────────────────────────

add("pipeline: a changed and a new file, scope .", { reply: ok() });
add("pipeline: the description is every argument after the scope, joined", { reply: ok(), args: [".", "add", "the", "second", "file"] });
add("pipeline: nothing to commit", { setup: [], reply: ok() });
add("pipeline: nothing to commit, and no key anywhere", { setup: [], env: { OPENAI_API_KEY: null }, reply: ok() });
add("pipeline: scope - commits what is staged, other changes are listed too", {
  args: ["-", "add b"],
  setup: [...CHANGES, "git add b.txt"],
  reply: ok(),
});
add("pipeline: scope - with nothing staged: the commit fails and the rest goes on", { args: ["-", "nothing staged"], setup: ["printf 'two\\n' >> a.txt"], reply: ok() });
add("pipeline: scope - with only an untracked file", { args: ["-", "untracked"], setup: ["printf 'bee\\n' > b.txt"], reply: ok() });
add("pipeline: the model proposes another type than the command's", { reply: ok({ type: "docs", scope: "readme" }) });
add("pipeline: SNOCOMMIT_NO_NATIVE=1 hands everything back", { env: { SNOCOMMIT_NO_NATIVE: "1" }, reply: ok(), handback: "SNOCOMMIT_NO_NATIVE=1" });
for (const cmd of ["chore", "docs", "fix", "refactor", "styles"]) {
  add("pipeline: a changed and a new file, scope .", { cmd, reply: ok({ type: cmd }) });
  add("usage: no arguments", { cmd, args: [], setup: [], after: [LOG], handback: THROWS });
}

// ── what goes into the prompt ──────────────────────────────────────────────

add("prompt: a description with quotes, backslashes, dollars, a line feed and other scripts", {
  args: [".", `it's "quoted" \\ $HOME \`id\``, "line\nfeed", "ünïcödé 日本 😀", "\u0001\t"],
  reply: ok(),
});
add("prompt: a description of one space", { args: [".", " "], reply: ok() });
add("prompt: --help and help are words of the description", { args: [".", "--help", "help"], reply: ok() });
add("prompt: more than five commits in the log", {
  setup: [...[2, 3, 4, 5, 6, 7].map((n) => `printf '${n}\\n' >> a.txt && git commit -q -am 'fix: step ${n}'`), "git push -q", ...CHANGES],
  reply: ok(),
});
add("prompt: a subject and an author with tabs, bars and other scripts", {
  setup: ["printf 'x\\n' >> a.txt && GIT_AUTHOR_NAME='Ünï | Cödé' git commit -q -am \"$(printf 'feat: a\\tb | c 日本')\"", "git push -q", ...CHANGES],
  reply: ok(),
});
add("prompt: a file with CRLF line ends", { setup: ["printf 'one\\r\\ntwo\\r\\n' > crlf.txt"], reply: ok() });
add("prompt: a deleted file", { setup: ["git rm -q a.txt", "printf 'bee\\n' > b.txt"], reply: ok() });
add("prompt: a binary file", { setup: ["printf 'a\\000b' > bin.dat"], reply: ok() });
add("prompt: twelve files", { setup: [...Array.from({ length: 12 }, (_, i) => `printf '${i}\\n' > f${String(i).padStart(2, "0")}.txt`)], reply: ok() });
// A new file f.txt of one line of n characters has a diff of 112 + n code
// units (its last line feed trimmed).
// head and tr, or printf with seq, mean the same on Linux, macOS and in Git Bash.
const oneLine = (n, ch = "x") => (ch === "x" ? `{ head -c ${n} /dev/zero | tr '\\000' x; echo; } > f.txt` : `{ printf '${ch}%.0s' $(seq ${n}); echo; } > f.txt`);
add("prompt: a diff of 2,000 code units is sent whole", { setup: [oneLine(1888)], reply: ok() });
add("prompt: a diff of 2,001 code units is cut to its first and last 1,000", { setup: [oneLine(1889)], reply: ok() });
add("prompt: a diff of 1,992 code units in 3,872 bytes is sent whole", { setup: [oneLine(1880, "é")], reply: ok() });
add("prompt: a diff of 2,001 code units of é is cut by code units", { setup: [oneLine(1889, "é")], reply: ok() });
add("prompt: a cut through a character of two code units", {
  setup: ["{ printf 'x%.0s' $(seq 887); printf '😀'; printf 'y%.0s' $(seq 1500); echo; } > f.txt"],
  reply: ok(),
  handback: "the TypeScript would send half a character (a lone surrogate), which the port does not reproduce",
});
add("prompt: a diff of 200,000 characters", { setup: [oneLine(200000)], reply: ok(), capture: "files" });
add("prompt: a file that is not UTF-8", {
  setup: ["printf 'caf\\351\\n' > latin1.txt"],
  reply: ok(),
  handback: "a git command printed bytes that are not UTF-8, which node replaces in its own way",
});
add("prompt: a file name git quotes (a space)", { setup: ["printf 'x\\n' > 'a b.txt'"], reply: ok(), handback: SHELL });
add("prompt: a file name with other scripts, core.quotePath off", { setup: ["git config core.quotePath false", "printf 'x\\n' > 'é日.txt'"], reply: ok() });
add("prompt: a file name with other scripts, quoted by git", { setup: ["printf 'x\\n' > 'é日.txt'"], reply: ok(), handback: SHELL });
add("prompt: a file name with a command in it, which the TypeScript runs", {
  setup: ["printf 'x\\n' > '$(touch${IFS}pwned)'"],
  reply: ok(),
  after: ["ls", ...AFTER],
  handback: SHELL,
});
add("prompt: a rename, whose status line the TypeScript's shell takes for a redirection", {
  setup: ["git mv a.txt c.txt"],
  reply: ok(),
  after: ["cat c.txt", ...AFTER],
  handback: SHELL,
});
add("prompt: ?? in a status line is a glob, and the shell expands it", {
  args: ["-", "glob"],
  setup: ["printf 'x\\n' > ab", "printf 'y\\n' > x.txt", "git add x.txt"],
  reply: ok(),
});
add("prompt: a file name with # and other characters the shell only reads as words", {
  setup: ["printf 'x\\n' > 'n#1=%~!.txt'", "printf 'y\\n' > '#lead'"],
  reply: ok(),
});

// ── the scope ──────────────────────────────────────────────────────────────

add("scope: a directory", { args: ["src", "add src"], setup: ["mkdir src", "printf 's\\n' > src/s.txt", ...CHANGES], reply: ok({ scope: "src" }) });
add("scope: two paths, split by the shell", { args: ["a.txt b.txt", "two files"], reply: ok() });
add("scope: a glob, expanded by the shell", { args: ["*.md", "notes"], setup: ["printf 'n\\n' > n.md", "printf 'm\\n' > m.md", ...CHANGES], reply: ok() });
add("scope: a path that does not exist", { args: ["nope", "missing"], reply: ok() });
add("scope: @ with a package.json here", { args: ["@", "the package"], files: { "package.json": '{"name":"pkg","version":"1.0.0"}\n' }, reply: ok() });
add("scope: @ from a directory with its own package.json", {
  args: ["@", "the inner package"],
  env: { PARITY_CD: "inner" },
  setup: ["mkdir inner", "printf '{\"name\":\"inner\"}' > inner/package.json", "printf 'i\\n' > inner/i.txt", ...CHANGES],
  reply: ok(),
});
add("scope: @ finds a package.json above the repository", {
  args: ["@", "above"],
  setup: ["printf '{\"name\":\"above\"}' > ../package.json", ...CHANGES],
  reply: ok(),
});
add("scope: @ with no package.json anywhere", { args: ["@", "none"], reply: ok(), handback: THROWS });
add("scope: . from a subdirectory", {
  env: { PARITY_CD: "sub" },
  setup: ["mkdir sub", "printf 's\\n' > sub/s.txt", ...CHANGES],
  reply: ok(),
});
add("scope: an option of git add", { args: ["-A", "all"], reply: ok(), handback: "an option to git add, not a path" });
add("scope: empty", { args: ["", "empty"], reply: ok(), handback: "an empty scope" });
add("scope: shell syntax, which the TypeScript runs", {
  args: ["a.txt;touch pwned", "injected"],
  reply: ok(),
  after: ["ls", ...AFTER],
  handback: SHELL,
});
add("scope: a path in quotes", { args: ["'a.txt'", "quoted"], reply: ok(), handback: SHELL });

// ── usage ──────────────────────────────────────────────────────────────────

add("usage: no arguments", { args: [], setup: [], after: [LOG], handback: THROWS });
add("usage: a scope and no description", { args: ["."], after: [LOG], handback: THROWS });
add("usage: an empty description", { args: [".", ""], after: [LOG], handback: THROWS });
add("usage: a colour flag among the arguments", { args: [".", "--no-color"], reply: ok(), handback: "supports-color reads colour flags from the arguments" });

// ── the key ────────────────────────────────────────────────────────────────

const rc = (text) => `printf '%s' '${text}' > <HOME>/.snocommitrc.json`;
add("key: from ~/.snocommitrc.json, before OPENAI_API_KEY", { setup: [rc(`{"OPENAI_API_KEY":"${RC_KEY}"}`), ...CHANGES], reply: ok() });
add("key: from ~/.snocommitrc.json, with spaces after it", { setup: [rc(`{"OPENAI_API_KEY":"${RC_KEY}   "}`), ...CHANGES], reply: ok() });
add("key: ~/.snocommitrc.json is not JSON", { setup: [rc("{oops"), ...CHANGES], reply: ok() });
add("key: ~/.snocommitrc.json has a key without sk-", { setup: [rc('{"OPENAI_API_KEY":"pk-0123456789abcdefghijkl"}'), ...CHANGES], reply: ok() });
add("key: ~/.snocommitrc.json has a key of 20 characters", { setup: [rc('{"OPENAI_API_KEY":"sk-0123456789abcdefg"}'), ...CHANGES], reply: ok() });
add("key: ~/.snocommitrc.json is a directory", { setup: ["mkdir <HOME>/.snocommitrc.json", ...CHANGES], reply: ok(), handback: "an rc file that is not a regular file" });
add("key: ~/.snocommitrc.json holds a number only JavaScript reads", {
  setup: [rc(`{"n":1e999,"OPENAI_API_KEY":"${RC_KEY}"}`), ...CHANGES],
  reply: ok(),
  handback: "an rc file that only JSON.parse reads",
});
add("key: ~/.snocommitrc.json with a comma JSON.parse refuses", { setup: [rc(`{"OPENAI_API_KEY":"${RC_KEY}",}`), ...CHANGES], reply: ok() });
add("key: OPENAI_API_KEY with a line feed at its end", { env: { OPENAI_API_KEY: `${KEY}\n` }, reply: ok(), handback: "a key with a character other than printable ASCII" });
add("key: OPENAI_API_KEY of 21 characters", { env: { OPENAI_API_KEY: "sk-0123456789abcdefgh" }, reply: ok() });
add("key: OPENAI_API_KEY too short, and no answer on stdin", { env: { OPENAI_API_KEY: "sk-short" }, reply: ok(), handback: ASKS });
add("key: none, and one typed on stdin", {
  env: { OPENAI_API_KEY: null },
  stdin: "sk-parity-typed-0123456789abcd\n",
  reply: ok(),
  after: ["cat <HOME>/.snocommitrc.json", ...AFTER],
  handback: ASKS,
});

// ── the model's answer ─────────────────────────────────────────────────────

add("model: a breaking change", { reply: ok({ is_BREAKING_CHANGE: true }) });
add("model: no scope", { reply: ok({ scope: "" }) });
add("model: a scope in parentheses already", { reply: ok({ scope: "(core)" }) });
add("model: a scope with one parenthesis at the end", { reply: ok({ scope: "core)" }) });
add("model: a scope that is one parenthesis", { reply: ok({ scope: "(" }) });
add("model: a scope with a line feed", { reply: ok({ scope: "a\nb" }), asked: true, handback: "a scope with a line feed, which the regular expression's . does not match" });
add("model: a title of 72 code units, an emoji counting two", { reply: ok({ title: `${"x".repeat(70)}😀` }) });
add("model: a title of 73 code units", { reply: ok({ title: `${"x".repeat(71)}😀` }), asked: true, handback: "zod rejects the answer" });
add("model: an empty title", { reply: ok({ title: "" }), asked: true, handback: "zod rejects the answer" });
add("model: an empty body", { reply: ok({ body: "" }), asked: true, handback: "zod rejects the answer" });
add("model: a type that is not one of the six", { reply: ok({ type: "perf" }), asked: true, handback: "zod rejects the answer" });
add("model: a field missing", { reply: answer({ type: "feat", scope: "", title: "t", body: "b" }), asked: true, handback: "zod rejects the answer" });
add("model: a field more", { reply: ok({ extra: [1, 2] }) });
add("model: JSON with space around it", { reply: { content: `\n  ${JSON.stringify(proposal())}  \n` } });
add("model: content that is not JSON", { reply: { content: "feat: plain text" }, asked: true, handback: THROWS });
add("model: no content", { reply: {}, asked: true, handback: THROWS });
add("model: a body of several lines and paragraphs", { reply: ok({ body: "First line.\n\n- one\n- two\n\nLast." }) });
add("model: double quotes, single quotes and ! in the message", { reply: ok({ title: `say "hi" to 'them'!`, body: `a "b" c\n"d"` }) });
add("model: a tab, other scripts and % in the message", { reply: ok({ title: "tabs\tand ünïcödé 日本 100%", body: "😀 #1" }) });
add("model: $ in the message, which the TypeScript's shell expands", { reply: ok({ body: "uses $TZ and ${GIT_AUTHOR_NAME}" }), asked: true, handback: SHELL });
add("model: a backquote in the message, which the TypeScript's shell runs", { reply: ok({ body: "run `echo injected`" }), asked: true, handback: SHELL });
add("model: a backslash in the message", { reply: ok({ title: "a \\n b" }), asked: true, handback: SHELL });
add("model: a NUL in the message", { reply: ok({ body: "a\u0000b" }), asked: true, handback: "a control character in the message" });
add("model: a line separator in the title", { reply: ok({ title: "one\u2028two" }) });
add("model: a carriage return in the message", { reply: ok({ body: "a\r\nb" }), asked: true, handback: "a control character in the message" });

// ── the service ────────────────────────────────────────────────────────────

const SERVICE = "the SDK retries, or throws in its own words";
add("service: 500 every time", { reply: { status: 500, body: '{"error":{"message":"boom","type":"server_error"}}' }, asked: true, handback: SERVICE });
add("service: 500, then an answer", { reply: [{ status: 500, body: "{}" }, ok()], asked: true, handback: SERVICE });
add("service: 401", { reply: { status: 401, body: '{"error":{"message":"Incorrect API key provided","type":"invalid_request_error","code":"invalid_api_key"}}' }, asked: true, handback: SERVICE });
add("service: 201 with an answer", { reply: { status: 201, body: JSON.stringify({ choices: [{ message: { content: JSON.stringify(proposal()) } }] }) }, asked: true, handback: SERVICE });
add("service: an answer that is not JSON", { reply: { status: 200, body: "<html>" }, asked: true, handback: SERVICE });
add("service: an answer as text/plain", { reply: { status: 200, type: "text/plain", body: JSON.stringify({ choices: [{ message: { content: JSON.stringify(proposal()) } }] }) }, asked: true, handback: SERVICE });
add("service: no choices", { reply: { status: 200, body: '{"choices":[]}' }, asked: true, handback: THROWS });
add("service: nobody listening", { env: { PARITY_BASE_URL: "http://127.0.0.1:1/v1" }, timeout: 60, handback: SERVICE });

// ── the address ────────────────────────────────────────────────────────────

add("address: OPENAI_BASE_URL with a slash at the end", { env: { PARITY_BASE_URL: "http://127.0.0.1:@PORT@/v1/" }, reply: ok() });
add("address: OPENAI_BASE_URL without /v1", { env: { PARITY_BASE_URL: "http://127.0.0.1:@PORT@" }, reply: ok() });
add("address: OPENAI_BASE_URL with a longer path", { env: { PARITY_BASE_URL: "http://127.0.0.1:@PORT@/proxy/openai/v1" }, reply: ok() });
add("address: OPENAI_BASE_URL with spaces around it", { env: { PARITY_BASE_URL: "  http://127.0.0.1:@PORT@/v1 " }, reply: ok(), handback: "an address in a form the port does not take" });

// ── git ────────────────────────────────────────────────────────────────────

add("git: not a repository", { repo: false, setup: ["printf 'x\\n' > x.txt"], reply: ok(), after: [LOG], handback: "not a git repository" });
add("git: the first commit of a repository, without a remote", {
  repo: false,
  setup: ["git init -q -b main", ...CHANGES],
  reply: ok(),
  after: [AFTER[0], AFTER[1], LOG],
});
add("git: no remote", { setup: ["git remote remove origin", ...CHANGES], reply: ok(), after: [AFTER[0], AFTER[1], LOG] });
add("git: the remote has moved on, and pull refuses to merge", {
  setup: ["git clone -q <ROOT>/remote.git <ROOT>/other", "cd <ROOT>/other && printf 'z\\n' > z.txt && git add z.txt && git commit -q -m 'feat: z' && git push -q", ...CHANGES],
  reply: ok(),
});
add("git: the remote has moved on, and pull rebases", {
  setup: ["git config pull.rebase true", "git clone -q <ROOT>/remote.git <ROOT>/other", "cd <ROOT>/other && printf 'z\\n' > z.txt && git add z.txt && git commit -q -m 'feat: z' && git push -q", ...CHANGES],
  reply: ok(),
});
/** A hook script of these lines (none with a single quote). */
const hook = (name, ...lines) => `printf '%s\\n' '#!/bin/sh' ${lines.map((line) => `'${line}'`).join(" ")} > .git/hooks/${name} && chmod +x .git/hooks/${name}`;
add("git: a pre-commit hook that refuses", { setup: [hook("pre-commit", "echo refused >&2", "exit 1"), ...CHANGES], reply: ok() });
add("git: a commit-msg hook sees the message and the key", {
  setup: [hook("commit-msg", 'cat "$1"', 'echo "key=${OPENAI_API_KEY-unset}"'), ...CHANGES],
  reply: ok(),
});
// A silent hook: how often git writes the index (and so runs it) depends on
// timing, the same command twice.
add("git: a post-index-change hook", { setup: [hook("post-index-change", "exit 0"), ...CHANGES], reply: ok(), handback: "git would run the hook for the binary's copy of the index too" });
add("git: another git holds the index lock", { setup: [...CHANGES, "touch .git/index.lock"], reply: ok(), handback: "the index is locked" });
add("git: a split index", { setup: ["git update-index --split-index", ...CHANGES], reply: ok(), handback: "a split index, whose shared part a copy would share" });
add("git: the temp directory inside the work tree", { env: { TMPDIR: "<CWD>/tmp" }, setup: ["mkdir tmp", ...CHANGES], reply: ok(), handback: "the binary's copy of the index would be among the changes" });
add("git: the temp directory inside the work tree, scope -", {
  args: ["-", "untracked"],
  env: { TMPDIR: "<CWD>/tmp" },
  setup: ["mkdir tmp", "printf 'bee\\n' > b.txt"],
  reply: ok(),
  handback: "the binary's copy of the index would be among the changes",
});
add("git: a linked worktree", {
  env: { PARITY_CD: "../wt" },
  setup: ["git worktree add -q -b side <ROOT>/wt", "cd <ROOT>/wt && printf 'w\\n' > w.txt"],
  reply: ok(),
  after: ["git -C <ROOT>/wt log --format='%h %s' side", "git -C <ROOT>/wt status --porcelain", LOG],
});

// ── the runtime ────────────────────────────────────────────────────────────

add("runtime: NODE_OPTIONS", { env: { NODE_OPTIONS: "--no-warnings" }, reply: ok(), handback: BUN });
add("runtime: a BUN_ variable", { env: { BUN_CONFIG_NO_CLEAR_TERMINAL_ON_RELOAD: "1" }, reply: ok(), handback: BUN });
add("runtime: BUN_INSTALL changes nothing", { env: { BUN_INSTALL: "<HOME>/.bun" }, reply: ok() });
add("runtime: FORCE_COLOR", { env: { FORCE_COLOR: "1" }, reply: ok(), handback: BUN });
// NO_PROXY too: bun's fetch, where it runs the JavaScript, would take the
// stand-in's address to the proxy and fail with the port in its message.
add("runtime: HTTP_PROXY", { env: { HTTP_PROXY: "http://127.0.0.1:1", NO_PROXY: "127.0.0.1" }, reply: ok(), handback: BUN });
add("runtime: OPENAI_ORG_ID", { env: { OPENAI_ORG_ID: "org-parity" }, reply: ok(), handback: BUN });
add("runtime: under bun", { env: { PARITY_RUNTIME: "bun" }, reply: ok() });
add("runtime: under bun, nothing to commit", { env: { PARITY_RUNTIME: "bun" }, setup: [], reply: ok() });
add("runtime: under bun, @ with a plain package.json", { env: { PARITY_RUNTIME: "bun" }, args: ["@", "the package"], files: { "package.json": '{"name":"pkg","private":true}' }, reply: ok() });
add("runtime: under bun, a .env file", { env: { PARITY_RUNTIME: "bun" }, files: { ".env": `OPENAI_API_KEY=sk-parity-dotenv-0123456789abc\n` }, reply: ok(), handback: BUN });
add("runtime: under bun, a package.json bun complains about", { env: { PARITY_RUNTIME: "bun" }, files: { "package.json": '{"name":"x","type":7}' }, reply: ok(), handback: BUN });
add("runtime: under bun, a tsconfig.json", { env: { PARITY_RUNTIME: "bun" }, files: { "tsconfig.json": "{ // comment\n}\n" }, reply: ok(), handback: BUN });
add("runtime: under bun, a bunfig.toml", { env: { PARITY_RUNTIME: "bun" }, files: { "bunfig.toml": "[run]\nsilent = true\n" }, reply: ok(), handback: BUN });

// ── on a terminal ──────────────────────────────────────────────────────────

const NO_TTY = "a pseudo-terminal, which Windows does not have";
const tty = (name, o) => add(`terminal: ${name}`, { tty: "stdout,stderr", skip: { win32: NO_TTY }, reply: ok(), ...o });
tty("the echo is blue");
tty("nothing to commit", { setup: [] });
tty("TERM=dumb", { env: { TERM: "dumb" } });
tty("no TERM", { env: { TERM: null } });
tty("no TERM, COLORTERM set", { env: { TERM: null, COLORTERM: "1" } });
tty("CI without a known service", { env: { CI: "1" } });
tty("CI on GitHub Actions", { env: { CI: "true", GITHUB_ACTIONS: "true", TERM: "dumb2" } });
tty("only stdout", { tty: "stdout" });
tty("only stderr", { tty: "stderr" });
tty("a git command that writes to stderr", { setup: [hook("pre-commit", "echo hook says >&2"), ...CHANGES] });
tty("no key: the question", { env: { OPENAI_API_KEY: null }, tty: true, stdin: [["(sk-...):", "\u0004"]], handback: ASKS, timeout: 30 });

// ═══════════════════════════ systems ═══════════════════════════
//
// The same cases run on Linux, macOS and Windows; rustify's parity runner
// reads "handback" and "skip" per system, keyed as node's process.platform
// names them.
//
// On Windows node's exec runs every command line through cmd.exe, not
// /bin/sh, with other quoting and no line feeds; the binary hands every
// invocation back there before it does anything (rs/src/commit.rs prepare),
// so there every case is meant to reach the JavaScript.
const WINDOWS = "every invocation goes back to the JavaScript on Windows, whose exec runs cmd.exe (rs/src/commit.rs prepare)";
for (const c of cases) if (typeof c.handback !== "string") c.handback = { ...c.handback, win32: WINDOWS };

// ═══════════════════════════ write ═══════════════════════════

const out = {
  env: {
    TZ: "UTC",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_AUTHOR_NAME: "Parity",
    GIT_AUTHOR_EMAIL: "parity@example.com",
    GIT_COMMITTER_NAME: "Parity",
    GIT_COMMITTER_EMAIL: "parity@example.com",
    GIT_AUTHOR_DATE: "2026-01-02T03:04:05Z",
    GIT_COMMITTER_DATE: "2026-01-02T03:04:05Z",
    OPENAI_API_KEY: KEY,
    PARITY_LOG: "<ROOT>/openai.log",
  },
  cases,
};
const text = JSON.stringify(out, null, 1) + "\n";
const target = path.join(HERE, "parity.json");
if (flags.has("--check")) {
  if (!fs.existsSync(target) || fs.readFileSync(target, "utf8") !== text) {
    console.error("rs/parity.json is not what rs/parity.gen.mjs writes; run `node rs/parity.gen.mjs`");
    process.exit(1);
  }
  console.error(`rs/parity.json is up to date (${cases.length} cases)`);
} else {
  fs.writeFileSync(target, text);
  const on = (system) => cases.filter((c) => !c.skip?.[system] && (typeof c.handback === "string" || c.handback?.[system])).length;
  const skipped = (system) => cases.filter((c) => c.skip?.[system]).length;
  console.error(`wrote rs/parity.json (${cases.length} cases; handed back on linux ${on("linux")}, darwin ${on("darwin")}, win32 ${on("win32")}; skipped on win32 ${skipped("win32")})`);
}
