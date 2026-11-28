#!/usr/bin/env node
// Runs one snocommit command against a stand-in for the OpenAI API.
//
//   node rs/parity/with-mock.mjs ts <command> [args...]      the TypeScript:
//                                    node dist/cli-<command>.js [args...]
//   node rs/parity/with-mock.mjs <dir> <command> [args...]   <dir>/<command>,
//                                    the binary under that command's name
//
// snocommit asks OpenAI for its commit message through the openai SDK, which
// takes its address from OPENAI_BASE_URL. This starts a server on 127.0.0.1
// that answers in OpenAI's place, runs the command with OPENAI_BASE_URL
// naming that server, and writes down every request the server received, so
// that a parity case compares what each implementation asked for and not
// only what it printed. The binary reads the same variable as the SDK does,
// and the JavaScript a binary hands back to inherits it: nothing here can
// reach the real service, and the wrapper refuses to start a command with any
// other address.
//
// What the case says, in variables the command does not get:
//
//   PARITY_LOG=<file>       where the requests go, one line each: method,
//                           path, Authorization, Content-Type, Accept and the
//                           body as it came, between tabs (required)
//   PARITY_OPENAI=<json>    what the server answers: one reply or a list of
//                           them used in turn, the last one repeating:
//                             { "content": "<text>" }  a chat completion whose
//                                                      message is that text
//                             { "status": 500, "body": "<text>",
//                               "type": "<content type>" }  any other answer
//                           (default: a completion with no content at all)
//   PARITY_BASE_URL=<url>   OPENAI_BASE_URL for the command, @PORT@ standing
//                           for the server's port (default
//                           http://127.0.0.1:@PORT@/v1); it has to name
//                           127.0.0.1 once trimmed
//   PARITY_CD=<dir>         start the command in this directory, relative to
//                           the case's
//   PARITY_RUNTIME=bun      put a directory first on PATH whose `node` is bun,
//                           so that the JavaScript runs under bun, as where
//                           node is a link to it (a symbolic link; on
//                           Windows a hard link, or a copy)
//   PARITY_RUNTIME=bun-hardlink   the same with a hard link of bun's own file
//                           named node (a copy where the temp directory is
//                           on another volume), which no link tells apart
//                           from node
//
// Every reply other than a completion carries `retry-after-ms: 1`, so that the
// SDK's retries (two after a 5xx or a 429) do not wait. The server sends no
// Date header, so that an error report that shows the headers is the same in
// every run.

import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "..", "..");
const isWindows = process.platform === "win32";
const [impl, command, ...args] = process.argv.slice(2);
if (!impl || !command) {
  console.error("usage: with-mock.mjs ts|<dir> <command> [args...]");
  process.exit(2);
}
const take = (name) => {
  const value = process.env[name];
  delete process.env[name];
  return value;
};
const logFile = take("PARITY_LOG");
const replySpec = take("PARITY_OPENAI");
const baseSpec = take("PARITY_BASE_URL") ?? "http://127.0.0.1:@PORT@/v1";
const cd = take("PARITY_CD");
const runtime = take("PARITY_RUNTIME");
if (!logFile) {
  console.error("with-mock: PARITY_LOG is not set");
  process.exit(2);
}
if (!baseSpec.trim().startsWith("http://127.0.0.1:")) {
  console.error(`with-mock: refusing OPENAI_BASE_URL ${JSON.stringify(baseSpec)}: only 127.0.0.1`);
  process.exit(2);
}
const replies = [].concat(replySpec ? JSON.parse(replySpec) : [{}]);
let turn = 0;

function completion(content) {
  return JSON.stringify({
    id: "chatcmpl-parity",
    object: "chat.completion",
    created: 0,
    model: "gpt-4o-2024-08-06",
    choices: [{ index: 0, message: { role: "assistant", content: content ?? null, refusal: null }, logprobs: null, finish_reason: "stop" }],
    usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
  });
}

const server = http.createServer((req, res) => {
  res.sendDate = false;
  const chunks = [];
  req.on("data", (chunk) => chunks.push(chunk));
  req.on("end", () => {
    // One line, the body as it came: both sides send JSON, which has no line
    // feed or tab of its own, and a path in it (the prompt's cwd) is spelt
    // once escaped, as the parity runner recognises it on Windows.
    const header = (name) => req.headers[name] ?? "(none)";
    const body = Buffer.concat(chunks).toString("utf8");
    fs.appendFileSync(logFile, `${[req.method, req.url, header("authorization"), header("content-type"), header("accept"), body].join("\t")}\n`);
    const reply = replies[Math.min(turn++, replies.length - 1)];
    if (reply.status === undefined) {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(completion(reply.content));
    } else {
      res.writeHead(reply.status, { "content-type": reply.type ?? "application/json", "retry-after-ms": "1" });
      res.end(reply.body ?? "");
    }
  });
});

/** The first file called `name` on PATH. */
function onPath(name) {
  for (const dir of (process.env.PATH ?? process.env.Path ?? "").split(path.delimiter)) {
    for (const ext of isWindows ? [".exe", ".cmd", ""] : [""]) {
      const file = path.join(dir, name + ext);
      try {
        if (fs.statSync(file).isFile()) return file;
      } catch {}
    }
  }
  return null;
}

let shims = null;
function runtimePath() {
  const pathKey = Object.keys(process.env).find((name) => name.toUpperCase() === "PATH") ?? "PATH";
  if (runtime === undefined) return {};
  if (runtime !== "bun" && runtime !== "bun-hardlink") {
    console.error(`with-mock: PARITY_RUNTIME=${runtime}: only bun or bun-hardlink`);
    process.exit(2);
  }
  const bun = onPath("bun");
  if (!bun) {
    console.error(`with-mock: PARITY_RUNTIME=${runtime}, but there is no bun on PATH`);
    process.exit(2);
  }
  shims = fs.mkdtempSync(path.join(os.tmpdir(), "parity-runtime-"));
  if (runtime === "bun-hardlink" && !isWindows) {
    const real = fs.realpathSync(bun);
    const node = path.join(shims, "node");
    try {
      fs.linkSync(real, node);
    } catch {
      fs.copyFileSync(real, node);
      fs.chmodSync(node, 0o755);
    }
  } else if (isWindows) {
    // A hard link where the temp directory is on bun's volume; else a copy.
    try {
      fs.linkSync(bun, path.join(shims, "node.exe"));
    } catch {
      fs.copyFileSync(bun, path.join(shims, "node.exe"));
    }
  } else fs.symlinkSync(fs.realpathSync(bun), path.join(shims, "node"));
  return { [pathKey]: `${shims}${path.delimiter}${process.env[pathKey] ?? ""}` };
}

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address();
  fs.writeFileSync(logFile, "", { flag: "a" });
  const env = { ...process.env, ...runtimePath(), OPENAI_BASE_URL: baseSpec.split("@PORT@").join(String(port)) };
  const [program, ...programArgs] =
    impl === "ts" ? ["node", path.join(repo, "dist", `cli-${command}.js`), ...args] : [path.join(impl, command + (isWindows ? ".exe" : "")), ...args];
  const child = spawn(program, programArgs, { stdio: "inherit", env, cwd: cd ? path.resolve(cd) : process.cwd() });
  const finish = (code) => {
    server.close();
    if (shims) fs.rmSync(shims, { recursive: true, force: true });
    process.exit(code);
  };
  child.on("error", (error) => {
    console.error(`with-mock: could not run ${program}: ${error.message}`);
    finish(127);
  });
  child.on("exit", (code, signal) => finish(code ?? 128 + (os.constants.signals[signal] ?? 0)));
});
