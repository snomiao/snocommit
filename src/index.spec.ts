import { getRecentCommits } from "./index";

describe("snocommit", () => {
  it("should be a function export by default", async () => {
    const mod = await import("./index");
    expect(typeof mod.default).toBe("function");
  });

  it("should export getRecentCommits helper", async () => {
    const mod = await import("./index");
    expect(typeof mod.getRecentCommits).toBe("function");
  });
});

describe("getRecentCommits", () => {
  it("should return a string", async () => {
    const result = await getRecentCommits();
    expect(typeof result).toBe("string");
  });

  it("should format commits correctly when commits exist", async () => {
    const result = await getRecentCommits(3);

    if (
      !result.includes("(no recent commits found)") &&
      !result.includes("(unable to read recent commits)")
    ) {
      const lines = result.split("\n");
      expect(lines.length).toBeGreaterThan(0);

      lines.forEach((line) => {
        expect(line).toMatch(/^- [a-f0-9]+ \| .+ \| \d{4}-\d{2}-\d{2} \| .+$/);
      });
    }
  });

  it("should handle when no commits are found", async () => {
    const result = await getRecentCommits(0);
    expect(result).toMatch(
      /\(no recent commits found\)|\(unable to read recent commits\)/,
    );
  });

  it("should accept custom count parameter", async () => {
    const result1 = await getRecentCommits(1);
    const result2 = await getRecentCommits(2);

    if (
      !result1.includes("(no recent commits found)") &&
      !result2.includes("(no recent commits found)")
    ) {
      const lines1 = result1.split("\n").filter((line) => line.trim());
      const lines2 = result2.split("\n").filter((line) => line.trim());

      expect(lines2.length).toBeGreaterThanOrEqual(lines1.length);
    }
  });
});
