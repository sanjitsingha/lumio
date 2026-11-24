import { describe, expect, it } from "vitest";
import { suggestCurrency } from "../src/utils/currency";

describe("suggestCurrency", () => {
  it("finishes a currency being typed", () => {
    expect(suggestCurrency("100 u")).toBe("100 usd");
    expect(suggestCurrency("100 us")).toBe("100 usd");
    expect(suggestCurrency("100 e")).toBe("100 eur");
    expect(suggestCurrency("100 d")).toBe("100 dollar");
    expect(suggestCurrency("100 rup")).toBe("100 rupee");
    expect(suggestCurrency("100 US")).toBe("100 USD");
    expect(suggestCurrency("100US")).toBe("100USD");
  });
  it("suggests where a whole currency goes", () => {
    expect(suggestCurrency("100 usd")).toBe("100 usd to inr");
    expect(suggestCurrency("100 USD")).toBe("100 USD to INR");
    expect(suggestCurrency("100 inr")).toBe("100 inr to usd");
    expect(suggestCurrency("100 dollars")).toBe("100 dollars to inr");
    expect(suggestCurrency("5 eur")).toBe("5 eur to inr");
  });
  it("goes on through ' to ' and the currency after it", () => {
    expect(suggestCurrency("100 usd ")).toBe("100 usd to inr");
    expect(suggestCurrency("100 usd t")).toBe("100 usd to inr");
    expect(suggestCurrency("100 usd to")).toBe("100 usd to inr");
    expect(suggestCurrency("100 usd to ")).toBe("100 usd to inr");
    expect(suggestCurrency("100 usd to e")).toBe("100 usd to eur");
    expect(suggestCurrency("100 usd to g")).toBe("100 usd to gbp");
  });
  it("leaves a lone number and everything else alone", () => {
    for (const q of ["", "100", "100 ", "chrome", "1 pen", "100 usd to inr", "100 usd to eur", "100 zzzz", "100 usd and more", "dollar shave club", "2 + 2"]) {
      expect(suggestCurrency(q), q).toBeNull();
    }
  });
});
