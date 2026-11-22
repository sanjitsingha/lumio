import { describe, expect, it } from "vitest";
import { answerConversion, formatMoney, parseConversion, relatedTerms, relevantResults } from "../src/utils/currency";

const CODES = ["USD", "INR", "EUR", "GBP", "JPY", "CAD", "PEN", "TRY", "ALL", "TOP", "PHP", "KRW"];
const parse = (text) => parseConversion(text, CODES);
const usdToInr = (amount = 1) => ({ amount, from: "USD", to: "INR" });

describe("parseConversion", () => {
  it.each([
    "$1",
    "1$",
    "1 usd",
    "1 USD",
    "1usd",
    "usd 1",
    "$1 inr",
    "$1 in inr",
    "inr $1",
    "1 dollar",
    "1 dollar in rupees",
    "1 dollar to rupee",
    "1 usd = ? inr",
    "1 usd -> inr",
    "usd to inr",
    "usd inr",
    "usdinr",
    "usd/inr",
    "usd2inr",
    "dollar to rupee",
    "dollar rate",
    "dollar rate in india",
    "what is inr value of $1",
    "what is the INR value of 1 USD?",
    "whats the value of $1 in rupees",
    "what's 1 dollar in inr",
    "how much is 1 dollar in rupees",
    "how many rupees in a dollar",
    "how many inr for 1 usd",
    "inr value of usd",
    "price of dollar in rupees",
    "convert 1 usd to inr",
    "convert $1 into rupees",
    "one dollar in rupees",
    "1 us dollar to indian rupee",
    "1 dollar ka rupee",
    "1 dollar kitna rupee hai",
    "aaj dollar ka rate kya hai",
    "1 डॉलर में रुपये".replace("में", "mein"),
  ])("understands %s", (text) => {
    expect(parse(text)).toEqual(usdToInr());
  });

  it.each([
    "1 doller",
    "1 dolar to rupes",
    "1 dollr in ruppee",
    "1 dallar in rupaya",
    "waht is inr vlaue of $1",
    "convrt 1 dollar to rupess",
    "hw mch is 1 dolar in rupiya",
  ])("forgives the typos in %s", (text) => {
    expect(parse(text)).toEqual(usdToInr());
  });

  it("reads amounts the way people write them", () => {
    expect(parse("$1,000")).toEqual(usdToInr(1000));
    expect(parse("1.5 usd")).toEqual(usdToInr(1.5));
    expect(parse("$.5")).toEqual(usdToInr(0.5));
    expect(parse("10k usd")).toEqual(usdToInr(10000));
    expect(parse("2m dollars")).toEqual(usdToInr(2000000));
    expect(parse("1 million dollars")).toEqual(usdToInr(1000000));
    expect(parse("five hundred dollars")).toEqual(usdToInr(500));
    expect(parse("1.5 lakh inr to usd")).toEqual({ amount: 150000, from: "INR", to: "USD" });
    expect(parse("₹1,00,000 in dollars")).toEqual({ amount: 100000, from: "INR", to: "USD" });
    expect(parse("2 crore rupees in usd")).toEqual({ amount: 20000000, from: "INR", to: "USD" });
    expect(parse("Rs. 500 to usd")).toEqual({ amount: 500, from: "INR", to: "USD" });
  });

  it("finds which currency the amount is in", () => {
    expect(parse("500 rs")).toEqual({ amount: 500, from: "INR", to: "USD" });
    expect(parse("how many dollars in 500 rupees")).toEqual({ amount: 500, from: "INR", to: "USD" });
    expect(parse("inr 500 to usd")).toEqual({ amount: 500, from: "INR", to: "USD" });
    expect(parse("inr to usd 500")).toEqual({ amount: 500, from: "INR", to: "USD" });
    expect(parse("€20 to gbp")).toEqual({ amount: 20, from: "EUR", to: "GBP" });
    expect(parse("rupees per dollar")).toEqual({ amount: 1, from: "USD", to: "INR" });
  });

  it("tells countries' dollars and rupees apart", () => {
    expect(parse("1 canadian dollar in inr")).toEqual({ amount: 1, from: "CAD", to: "INR" });
    expect(parse("1 canadian dolar")).toEqual({ amount: 1, from: "CAD", to: "INR" });
    expect(parse("100 hong kong dollars to usd")).toEqual({ amount: 100, from: "HKD", to: "USD" });
    expect(parse("100 pakistani rupees in inr")).toEqual({ amount: 100, from: "PKR", to: "INR" });
    expect(parse("C$5 to usd")).toEqual({ amount: 5, from: "CAD", to: "USD" });
  });

  it("uses codes that are also words only when they are clearly currencies", () => {
    expect(parse("1 pen")).toBeNull();
    expect(parse("5 top")).toBeNull();
    expect(parse("100 try")).toBeNull();
    expect(parse("100 TRY")).toEqual({ amount: 100, from: "TRY", to: "INR" });
    expect(parse("100 pen to usd")).toEqual({ amount: 100, from: "PEN", to: "USD" });
    expect(parse("100 won")).toBeNull();
    expect(parse("100 won in inr")).toEqual({ amount: 100, from: "KRW", to: "INR" });
  });

  it.each([
    "",
    "$",
    "1",
    "42",
    "usd",
    "dollar",
    "rupee",
    "to",
    "chrome",
    "1 password",
    "2 days",
    "1 pond",
    "1 royal",
    "1 diner",
    "5 collars",
    "10 points",
    "dollar shave club",
    "pound cake",
    "euro truck simulator",
    "i won 100 dollars",
    "1 2 usd",
    "-5 usd",
    "10% of usd",
    "2+2",
  ])("leaves %j alone", (text) => {
    expect(parse(text)).toBeNull();
  });

  it("keeps answering while the next word is being typed", () => {
    expect(parse("1 usd t")).toEqual(usdToInr());
    expect(parse("1 usd to")).toEqual(usdToInr());
    expect(parse("1 usd to i")).toEqual(usdToInr());
    expect(parse("1 usd shave")).toBeNull();
  });
});

describe("parseConversion, the wider world", () => {
  const ALL = "AED AFN ALL AMD ANG AOA ARS AUD AWG AZN BAM BBD BDT BGN BHD BIF BMD BND BOB BRL BSD BTN BWP BYN BZD CAD CDF CHF CLF CLP CNH CNY COP CRC CUP CVE CZK DJF DKK DOP DZD EGP ERN ETB EUR FJD FKP FOK GBP GEL GGP GHS GIP GMD GNF GTQ GYD HKD HNL HRK HTG HUF IDR ILS IMP INR IQD IRR ISK JEP JMD JOD JPY KES KGS KHR KID KMF KRW KWD KYD KZT LAK LBP LKR LRD LSL LYD MAD MDL MGA MKD MMK MNT MOP MRU MUR MVR MWK MXN MYR MZN NAD NGN NIO NOK NPR NZD OMR PAB PEN PGK PHP PKR PLN PYG QAR RON RSD RUB RWF SAR SBD SCR SDG SEK SGD SHP SLE SLL SOS SRD SSP STN SYP SZL THB TJS TMT TND TOP TRY TTD TVD TWD TZS UAH UGX USD UYU UZS VES VND VUV WST XAF XCD XCG XDR XOF XPF YER ZAR ZMW ZWG ZWL".split(" ");
  const pair = (text) => {
    const r = parse(text);
    return r ? `${r.amount} ${r.from} ${r.to}` : null;
  };

  it("knows every currency of the service", () => {
    expect(ALL).toHaveLength(166);
    for (const code of ALL) expect(parseConversion(`100 ${code} to usd`, ALL)?.from, code).toBe(code);
    for (const code of ALL) expect(parseConversion(`1 usd to ${code}`, ALL)?.to, code).toBe(code);
  });

  it.each([
    ["1 indian rupees to nepali", "1 INR NPR"],
    ["1 inr to nepali", "1 INR NPR"],
    ["1 rupee in nepal", "1 INR NPR"],
    ["100 inr in nepal currency", "100 INR NPR"],
    ["100 nepali rupees to indian", "100 NPR INR"],
    ["1 lakh nepali to inr", "100000 NPR INR"],
    ["nepal ka rupee", "1 NPR INR"],
    ["nepali rupee", "1 NPR INR"],
    ["nepal ka rupee rate", "1 NPR INR"],
    ["currency of nepal", "1 NPR INR"],
    ["100 inr to bangladesh", "100 INR BDT"],
    ["usd to dubai money", "1 USD AED"],
    ["japan currency to inr", "1 JPY INR"],
    ["1000 rupees in thai money", "1000 INR THB"],
    ["1 dollar in saudi", "1 USD SAR"],
    ["500 rupees in uk", "500 INR GBP"],
    ["1 inr to us", "1 INR USD"],
    ["50 usd in south africa", "50 USD ZAR"],
    ["100 aed to sri lanka", "100 AED LKR"],
    ["1 dollar in trinidad and tobago", "1 USD TTD"],
  ])("places: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["1 indain rupee to nepali", "1 INR NPR"],
    ["1 inr to nepli", "1 INR NPR"],
    ["1 inr to nepall", "1 INR NPR"],
    ["100 ruppes to bangaldesh", "100 INR BDT"],
    ["1 dolar in inida", "1 USD INR"],
    ["1 usd to japn", "1 USD JPY"],
    ["1 eur to thialand", "1 EUR THB"],
    ["1 doller to malaysiya", "1 USD MYR"],
    ["1 royal to inr", "1 SAR INR"],
    ["1 usdd to inrr", "1 USD INR"],
    ["1 uds to irn", "1 USD INR"],
    ["10 gpb in rupees", "10 GBP INR"],
    ["waht is 1 dolar in rupess plz", "1 USD INR"],
  ])("typos: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["$100 usd to inr", "100 USD INR"],
    ["USD$100 in inr", "100 USD INR"],
    ["1usdtoinr", "1 USD INR"],
    ["usdtoinr", "1 USD INR"],
    ["dollartorupee", "1 USD INR"],
    ["1 usd 2 inr", "1 USD INR"],
    ["usd vs inr", "1 USD INR"],
    ["1 usd => inr", "1 USD INR"],
    ["500/- to usd", "500 INR USD"],
    ["100 in inr", "100 USD INR"],
    ["rs 500/- to usd", "500 INR USD"],
    ["1,5 eur to usd", "1.5 EUR USD"],
    ["1.000,50 eur to usd", "1000.5 EUR USD"],
    ["1 000 usd to inr", "1000 USD INR"],
    ["twenty five dollars in rupees", "25 USD INR"],
    ["two hundred fifty dollars", "250 USD INR"],
    ["1 crore 50 lakh inr to usd", "15000000 INR USD"],
    ["do dollar ka rupee", "2 USD INR"],
    ["ek hazar dollar ka rupee", "1000 USD INR"],
    ["50 cents in rupees", "0.5 USD INR"],
    ["50 paise to usd", "0.5 INR USD"],
    ["एक डॉलर में कितने रुपये", "1 USD INR"],
    ["hey bro what would 100 usd be in inr right now", "100 USD INR"],
    ["hey bro 100 usd in inr thanks", "100 USD INR"],
    ["can you please tell me 1 dollar in rupees", "1 USD INR"],
    ["100rs to $", "100 INR USD"],
    ["1000usd to inr", "1000 USD INR"],
    ["1.5usd in inr", "1.5 USD INR"],
    ["nepal ka 100 rupee india me kitna", "100 NPR INR"],
    ["india ka 1 rupee nepal me", "1 INR NPR"],
    ["2000 euro to inr", "2000 EUR INR"],
    ["1 usd to inr and eur", "1 USD INR"],
    ["usd to usd", "1 USD USD"],
  ])("phrasing: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    "india",
    "nepal",
    "india vs pakistan",
    "india to nepal",
    "usa to india",
    "100 nepali",
    "5 chinese",
    "trip to nepal",
    "flight from usa to india",
    "5 star hotels in dubai",
    "100 dollars to buy a laptop in india",
    "gold rate in india",
    "1 usd to xyzzy",
    "1 dollar in hindi",
    "tell us",
    "constructor",
    "euro 2024",
    "1 dollar walk",
    "pound to kg",
    "1 pound to kg",
    "5 pounds in grams",
    "1 pound to oz",
  ])("leaves %j alone", (text) => {
    expect(parse(text)).toBeNull();
  });

  it("names the currency as soon as it is clear while typing", () => {
    expect(pair("1 usd to nep")).toBe("1 USD NPR");
    expect(pair("1 usd to bangl")).toBe("1 USD BDT");
  });
});

describe("answerConversion", () => {
  const saved = { base: "USD", updatedAt: 1795305600, rates: { USD: 1, INR: 92.8891, EUR: 0.9113, JPY: 150 } };

  it("answers from the saved rates, across any two currencies", () => {
    expect(answerConversion("$1", saved)).toEqual({
      amount: 1,
      from: "USD",
      to: "INR",
      fromName: "US Dollar",
      toName: "Indian Rupee",
      source: "$1",
      answer: "₹92.89",
      rate: "1 USD = ₹92.89",
      updated: `Updated ${new Date(saved.updatedAt * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", timeZone: "UTC" })}`,
    });
    expect(answerConversion("100000 usd", saved).answer).toBe("₹92,88,910.00");
    expect(answerConversion("1 eur to inr", saved).answer).toBe("₹101.93");
    expect(answerConversion("100 rs", saved).answer).toBe("$1.08");
    expect(answerConversion("1 rs", saved).answer).toBe("$0.01077");
  });

  it("never invents an answer without rates", () => {
    const pending = answerConversion("$1", null);
    expect(pending.answer).toBeNull();
    expect(pending.to).toBe("INR");
    expect(pending.rate).toBeNull();
    expect(answerConversion("1 gbp", saved).answer).toBeNull();
  });

  it("is null for anything else", () => {
    expect(answerConversion("chrome", saved)).toBeNull();
  });
});

describe("results related to a conversion", () => {
  const result = (title, score = 1, kind = "file") => ({ id: `${kind}:${title}`, kind, title, score });
  const titles = (results) => results.map((r) => r.title);

  it("looks for the currencies' codes and everyday names", () => {
    expect(relatedTerms("USD", "INR")).toEqual(["usd", "dollar", "inr", "rupee", "currency"]);
    expect(relatedTerms("CAD", "KRW")).toEqual(["dollar", "krw", "currency"]);
  });

  it("keeps only names that contain a currency as a whole word", () => {
    const response = {
      sections: [
        {
          results: [
            result("Dollar invoice.pdf"),
            result("budget_usd.xlsx"),
            result("Currency Converter", 1, "app"),
            result("Dollhouse.png"),
            result("Busdriver.txt"),
            result("Inrush notes.txt"),
            result("Search Google for “usd”", 9, "web"),
          ],
        },
      ],
    };
    expect(titles(relevantResults([response], "USD", "INR"))).toEqual(["Dollar invoice.pdf", "budget_usd.xlsx", "Currency Converter"]);
  });

  it("leaves out source code and folders that happen to be named after currencies", () => {
    const response = {
      sections: [
        {
          results: [
            result("currency.js"),
            result("currency.rs"),
            result("currency.test.js"),
            result("currency", 1, "folder"),
            result("Currency rates.csv"),
          ],
        },
      ],
    };
    expect(titles(relevantResults([response], "USD", "INR"))).toEqual(["Currency rates.csv"]);
  });

  it("does not treat everyday words as currencies", () => {
    const response = { sections: [{ results: [result("We won.txt"), result("all photos.zip"), result("big bucks.mp4")] }] };
    expect(relevantResults([response], "KRW", "ALL")).toEqual([]);
    expect(relevantResults([response], "USD", "INR")).toEqual([]);
  });

  it("lists each result once, the best first, and only a few", () => {
    const a = { sections: [{ results: [result("usd a.txt", 1), result("usd b.txt", 4)] }] };
    const b = { sections: [{ results: [result("usd b.txt", 4), result("usd c.txt", 3), result("usd d.txt", 2)] }] };
    expect(titles(relevantResults([a, null, b], "USD", "INR"))).toEqual(["usd b.txt", "usd c.txt", "usd d.txt"]);
  });
});

describe("formatMoney", () => {
  it("groups rupees the Indian way and keeps small values readable", () => {
    expect(formatMoney(100000, "INR")).toBe("₹1,00,000.00");
    expect(formatMoney(100000, "USD")).toBe("$100,000.00");
    expect(formatMoney(0.010765, "USD")).toBe("$0.01077");
    expect(formatMoney(5, "USD", { whole: true })).toBe("$5");
    expect(formatMoney(1.5, "USD", { whole: true })).toBe("$1.50");
  });
});
