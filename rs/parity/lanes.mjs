#!/usr/bin/env node
// Splits a cases file into lanes, for parity runs side by side (the runner
// runs one case at a time, and a case of snocommit's starts git, a shell and
// a server several times):
//
//   node rs/parity/lanes.mjs <cases.json> <lanes> <directory>
//
// writes <directory>/lane-0.json … lane-<lanes - 1>.json, each with the
// file's own settings and every <lanes>-th case, and beside each a file
// lane-<n>.commands naming the commands its cases run (`[feat] …` is a case
// of feat): the hand-back check runs once per command, since each command
// hands back to a JavaScript file of its own. The runner takes <REPO> from
// the directory it is started in, not from where the cases file is, so a
// lane runs its cases as rs/parity.json would.

import fs from "node:fs";
import path from "node:path";

const [file, count, dir] = process.argv.slice(2);
const lanes = Number(count);
if (!file || !dir || !Number.isInteger(lanes) || lanes < 1) {
  console.error("usage: lanes.mjs <cases.json> <lanes> <directory>");
  process.exit(2);
}
const spec = JSON.parse(fs.readFileSync(file, "utf8"));
fs.mkdirSync(dir, { recursive: true });
for (let lane = 0; lane < lanes; lane++) {
  const cases = spec.cases.filter((_, index) => index % lanes === lane);
  fs.writeFileSync(path.join(dir, `lane-${lane}.json`), JSON.stringify({ ...spec, cases }));
  const commands = [...new Set(cases.map((c) => /^\[([a-z]+)\] /.exec(c.name)?.[1]).filter(Boolean))];
  fs.writeFileSync(path.join(dir, `lane-${lane}.commands`), `${commands.join(" ")}\n`);
}
console.log(`${spec.cases.length} cases in ${lanes} lane(s)`);
