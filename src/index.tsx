import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { execa } from "execa";
import { packageUp } from "package-up";
import { DIE } from "phpdie";
import snorun from "snorun";
import zChatCompletion from "z-chat-completion";
import z from "zod";
import commitTypes from "./commitTypes";
import { promptT } from "./promptT";

type Type = (typeof commitTypes)[number];
type Scope = "-" | "." | "@" | ":" | (string & {});

export const zSnocommitRC = z.object({
  OPENAI_API_KEY: z.string().regex(/^sk-/),
});

export default async function snocommit({
  type,
  scope,
  desc,
}: {
  type: Type;
  scope: Scope;
  desc: string;
}) {
  if (!desc) {
    throw new Error("Missing description (desc)");
  }

  await gitAddByScope(scope);
  const stagedFileChanges = await getStagedFileChanges();
  if (stagedFileChanges.length === 0) {
    console.log("No files staged to commit.");
    return;
  }

  const gitDiffMap = await getTruncatedGitDiffOfFiles(stagedFileChanges);
  const commitMessage = await generateCommitMessage(
    type,
    scope,
    desc,
    gitDiffMap,
  );

  await gitCommit(commitMessage);
  await gitPullAndPush();

  console.log("Commit pushed successfully.");
}

async function gitAddByScope(scope: Scope): Promise<void> {
  if (scope === "-") {
    return; // do nothing
  }
  if (scope === ".") {
    await snorun("git add .");
    return;
  }
  if (scope === "@") {
    const pkgPath =
      (await packageUp({ cwd: process.cwd() })) ||
      DIE`No package.json found for "@" scope.`;
    const pkgDir = path.dirname(pkgPath);
    await snorun(`git add ${pkgDir}`);
    return;
  }
  // assume it's a file or folder
  await snorun(`git add ${scope}`);
}

async function getStagedFileChanges(): Promise<string[]> {
  const stdout = await snorun("git status --porcelain").stdout;
  return stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

async function getTruncatedGitDiffOfFiles(
  files: string[],
): Promise<Record<string, string>> {
  const diffs: Record<string, string> = {};
  for (const file of files) {
    const stdout = await snorun(`git diff --cached -- ${file}`).stdout;
    if (stdout.length > 2000) {
      diffs[file] =
        stdout.slice(0, 1000) +
        "\n\n... (diff truncated) ...\n\n" +
        stdout.slice(-1000);
    } else {
      diffs[file] = stdout;
    }
  }
  return diffs;
}

async function generateCommitMessage(
  type: Type,
  scope: Scope,
  desc: string,
  diffMap: Record<string, string>,
): Promise<string> {
  process.env.OPENAI_API_KEY = await getOpenAIApiKey(); // ensure it's set

  const diff = Object.entries(diffMap)
    .map(([file, diff]) => `--- a/${file}\n+++ b/${file}\n${diff}`)
    .join("\n");

  const scopeString = scope && scope !== "." ? `(${scope})` : "";

  // todo: show spinner when generating
  console.log("Generating commit message...");

  const generated = await zChatCompletion({
    type: z.enum(commitTypes),
    scope: z.string(),
    title: z.string().min(1).max(72),
    body: z.string().min(1).max(200),
    is_BREAKING_CHANGE: z.boolean(),
  })`
Assist me in generating a conventional commit message based on the following information.

cwd: ${process.cwd()}
Suggested Commit Type: ${type}
Suggested Scope: ${scopeString || "none"}
Short Description Provided by user: ${desc}

Modified Files:
${Object.keys(diffMap).join("\n")}

Detailed Git Diff:
\`\`\`diff
${diff}
\`\`\`
  `;

  return `${generated.type}${generated.scope?.replace(
    /^\(?(.+?)\)?$/,
    "($1)",
  )}${generated.is_BREAKING_CHANGE ? "!" : ""}: ${generated.title}\n\n${
    generated.body
  }`; // raw
  // return `${type}${scopeString}: ${desc}`; // raw
}

async function gitCommit(message: string): Promise<void> {
  console.log("Committing with message:\n", message);
  await execa("git", ["commit", "-m", message]);
}

async function gitPullAndPush(): Promise<void> {
  await snorun("git pull --rebase");
  await snorun("git push");
}

async function getOpenAIApiKey(): Promise<string> {
  // 1. check process.env.OPENAI_API_KEY
  let key = await validApiKey(process.env.OPENAI_API_KEY);
  if (key) return key;

  // 2. check ~/.snocommitrc.json
  const home = process.env.HOME || process.env.USERPROFILE;
  if (home) {
    try {
      const rcPath = path.join(home, ".snocommitrc.json");
      const rcFile = await readFile(rcPath, "utf-8");
      const rc = zSnocommitRC.parse(JSON.parse(rcFile));
      key = await validApiKey(rc.OPENAI_API_KEY);
      if (key) return key;
    } catch (_error) {
      // ignore error if file doesn't exist or is invalid
    }
  }

  // 3. ask user to paste it
  key = await validApiKey(
    await promptT`Please enter your OpenAI API Key (sk-...):`,
  );
  if (key) {
    // save to ~/.snocommitrc.json
    if (home) {
      const rcPath = path.join(home, ".snocommitrc.json");
      const rc = { OPENAI_API_KEY: key };
      await writeFile(rcPath, JSON.stringify(rc, null, 2), "utf-8");
      console.log(`Saved OpenAI API Key to ${rcPath}`);
    }
    return key;
  }

  // 4. if not found, die
  DIE(
    "No valid OPENAI_API_KEY provided. Please set it as an environment variable or in ~/.snocommitrc.json",
  );
}

async function validApiKey(key?: string): Promise<string | null> {
  if (!key) return null;
  try {
    // Use a simple validation - check if key starts with sk- and has reasonable length
    if (key.startsWith("sk-") && key.length > 20) {
      return key.trim();
    }
  } catch (_error) {
    // ignore validation errors
  }
  return null;
}
