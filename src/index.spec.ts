import { describe, expect, it } from "bun:test";
import snocommit from "./index.tsx";

describe("snocommit", () => {
  it("should be a function", () => {
    expect(typeof snocommit).toBe("function");
  });

  // TODO: Add more comprehensive tests
  // Note: This function interacts with git and OpenAI API,
  // so tests may require mocking or integration setup
});
