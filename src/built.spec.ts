import { existsSync } from "node:fs";
import path from "node:path";

describe("built dist files", () => {
  const distDir = path.resolve("dist");

  it("should have built dist/index.js", () => {
    expect(existsSync(path.join(distDir, "index.js"))).toBe(true);
  });

  it("should have built dist/cli.js", () => {
    expect(existsSync(path.join(distDir, "cli.js"))).toBe(true);
  });

  it("should have built CLI command files", () => {
    const expectedCLIs = [
      "cli-chore.js",
      "cli-docs.js",
      "cli-feat.js",
      "cli-fix.js",
      "cli-refactor.js",
      "cli-styles.js",
    ];

    expectedCLIs.forEach((file) => {
      expect(existsSync(path.join(distDir, file))).toBe(true);
    });
  });

  it("should export snocommit function from dist/index.js", async () => {
    const { default: snocommit } = await import("../dist/index.js");
    expect(typeof snocommit).toBe("function");
  });

  it("should export getRecentCommits from dist/index.js", async () => {
    const { getRecentCommits } = await import("../dist/index.js");
    expect(typeof getRecentCommits).toBe("function");
  });

  it("built index.js should work independently", async () => {
    const { getRecentCommits } = await import("../dist/index.js");
    const result = await getRecentCommits(1);
    expect(typeof result).toBe("string");
  });
});
