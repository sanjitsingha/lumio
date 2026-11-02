import { describe, expect, it } from "vitest";
import { calculate } from "../src/utils/calc";

const answer = (text) => calculate(text)?.answer ?? null;

describe("calculate", () => {
  it("answers the basic operations", () => {
    expect(answer("2+2")).toBe("4");
    expect(answer("4/8")).toBe("0.5");
    expect(answer("3*6+6-45")).toBe("-21");
    expect(answer(" 10 - 2 * 3 ")).toBe("4");
  });

  it("follows precedence, parentheses and powers", () => {
    expect(answer("(1+2)*3")).toBe("9");
    expect(answer("2^3^2")).toBe("512");
    expect(answer("-2^2")).toBe("-4");
    expect(answer("2^-1")).toBe("0.5");
    expect(answer("2**10")).toBe("1024");
    expect(answer("-(3+4)*2")).toBe("-14");
  });

  it("handles decimals, typographic operators and float noise", () => {
    expect(answer(".5+1.25")).toBe("1.75");
    expect(answer("0.1+0.2")).toBe("0.3");
    expect(answer("6×7")).toBe("42");
    expect(answer("9÷4")).toBe("2.25");
    expect(answer("5−8")).toBe("-3");
  });

  it("writes the expression with spaced operators and signs on their numbers", () => {
    expect(calculate("3*6+6-45").expression).toBe("3 × 6 + 6 - 45");
    expect(calculate("2+4").expression).toBe("2 + 4");
    expect(calculate("-2*(-3+4)/.5").expression).toBe("-2 × (-3 + 4) ÷ 0.5");
    expect(calculate("2**10").expression).toBe("2^10");
  });

  it("stays quiet for anything that is not a finished sum", () => {
    for (const text of ["", "  ", "42", "-5", "(5)", "2+", "(1+2", "1+2)", "2 2", "abc", "2+a", "notes.txt", "v1.2", "1/0", "0/0"]) {
      expect(calculate(text), text).toBeNull();
    }
  });
});
