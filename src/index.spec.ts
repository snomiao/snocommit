import { describe, expect, it } from "bun:test";

describe("snocommit", () => {
  it("should be a function export by default", async () => {
    const mod = await import("./index.tsx");
    expect(typeof mod.default).toBe("function");
  });

  it("should export getRecentCommits helper", async () => {
    const mod = await import("./index.tsx");
    expect(typeof mod.getRecentCommits).toBe("function");
  });
});
