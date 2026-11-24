import { beforeAll, describe, expect, it } from "vitest";
import { answerTime, differenceText, findPlace, loadPlaces, offsetMinutes, parseTimeQuery, suggestTime } from "../src/utils/worldTime";

beforeAll(loadPlaces);

// 14:46 UTC on a winter day: India 20:16, Dubai 18:46, New York 09:46, London 14:46
const winter = new Date("2026-01-15T14:46:00Z");
const summer = new Date("2026-07-15T14:46:00Z");

describe("offsetMinutes", () => {
  it("reads the zone's own offset, daylight saving included", () => {
    expect(offsetMinutes("Asia/Kolkata", winter)).toBe(330);
    expect(offsetMinutes("Asia/Dubai", winter)).toBe(240);
    expect(offsetMinutes("America/New_York", winter)).toBe(-300);
    expect(offsetMinutes("America/New_York", summer)).toBe(-240);
    expect(offsetMinutes("Europe/London", summer)).toBe(60);
    expect(offsetMinutes("UTC", winter)).toBe(0);
  });
});

describe("parseTimeQuery", () => {
  it("understands the usual ways to ask", () => {
    for (const q of ["what is the time in dubai", "What's the time in Dubai?", "what time is it in dubai now", "current time in dubai", "time in dubai", "time dubai", "dubai time", "dubai time now", "time in DUBAI right now"]) {
      expect(parseTimeQuery(q)?.zone, q).toBe("Asia/Dubai");
    }
  });
  it("knows accents, spaces and short names", () => {
    expect(parseTimeQuery("time in São Paulo")?.zone).toBe("America/Sao_Paulo");
    expect(parseTimeQuery("time in new   york")?.zone).toBe("America/New_York");
    expect(parseTimeQuery("pst time")?.zone).toBe("America/Los_Angeles");
    expect(parseTimeQuery("time in IST")?.label).toBe("IST");
  });
  it("shows nothing when no place is named", () => {
    for (const q of ["time", "what time is it", "what is the time now", "current time", "time now", "time here"]) expect(parseTimeQuery(q), q).toBeUndefined();
  });
  it("shows nothing while the place is still being typed", () => {
    for (const q of ["time in", "time in ", "what is the time in", "current time at", "time for", "dubai ka", "time of"]) expect(parseTimeQuery(q), q).toBeUndefined();
  });
  it("leaves everything else alone", () => {
    for (const q of ["time machine", "time zone settings", "timer", "bedtime", "dubai", "chrome", "time in qqqqqzzzzz", "5 minutes to hours", "what is the weather in dubai"]) {
      expect(parseTimeQuery(q), q).toBeUndefined();
    }
  });
});

describe("every place, with typing mistakes", () => {
  const zoneOf = (q) => parseTimeQuery(q)?.zone;
  it("knows countries, cities and states", () => {
    expect(zoneOf("time in afghanistan")).toBe("Asia/Kabul");
    expect(zoneOf("time in lahore")).toBe("Asia/Karachi");
    expect(zoneOf("time in siliguri")).toBe("Asia/Kolkata");
    expect(zoneOf("time in texas")).toBe("America/Chicago");
    expect(zoneOf("time in queensland")).toBe("Australia/Brisbane");
    expect(zoneOf("time in tuvalu")).toBe("Pacific/Funafuti");
    expect(zoneOf("time in bombay")).toBe("Asia/Kolkata");
    expect(zoneOf("time in ivory coast")).toBe("Africa/Abidjan");
    expect(zoneOf("time in cote divoire")).toBe("Africa/Abidjan");
  });
  it("forgives misspellings, missing spaces and unfinished names", () => {
    for (const [typed, zone] of [
      ["afganistan", "Asia/Kabul"], ["afghanstan", "Asia/Kabul"], ["afgh", "Asia/Kabul"], ["dubia", "Asia/Dubai"],
      ["duabi", "Asia/Dubai"], ["new yrok", "America/New_York"], ["newyork", "America/New_York"], ["los angelas", "America/Los_Angeles"],
      ["tokio", "Asia/Tokyo"], ["singpore", "Asia/Singapore"], ["moscw", "Europe/Moscow"], ["lond", "Europe/London"],
    ]) {
      expect(zoneOf(`time in ${typed}`), typed).toBe(zone);
    }
  });
  it("shows the real name of a misspelt place", () => {
    expect(parseTimeQuery("time in afganistan").label).toBe("Afghanistan");
    expect(parseTimeQuery("time in dubia").label).toBe("Dubai");
  });
  it("picks the right one of several places with a name", () => {
    expect(zoneOf("time in paris")).toBe("Europe/Paris");
    expect(zoneOf("time in paris texas")).toBe("America/Chicago");
    expect(zoneOf("time in london canada")).toBe("America/Toronto");
    expect(zoneOf("time in the hague")).toBe("Europe/Amsterdam");
  });
  it("forgives a mistake in the question too, in English and Hinglish", () => {
    for (const q of ["tiem in dubai", "what is the tiem in dubai", "dubai ka time kya hai", "dubai me abhi kitne baje hai", "dubai samay", "time zone dubai", "dubai time right now please"]) {
      expect(zoneOf(q), q).toBe("Asia/Dubai");
    }
  });
  it("does not take near misses of everyday words for places", () => {
    for (const q of ["time table", "time tracker", "time machine", "time management", "time settings"]) expect(parseTimeQuery(q), q).toBeUndefined();
  });
});

describe("suggestTime", () => {
  it("goes on from time to a place, keeping what was typed", () => {
    expect(suggestTime("time")).toBe("time in New York");
    expect(suggestTime("time ")).toBe("time in New York");
    expect(suggestTime("time i")).toBe("time in New York");
    expect(suggestTime("Time in")).toBe("Time in New York");
    expect(suggestTime("time in ")).toBe("time in New York");
  });
  it("finishes an unfinished place", () => {
    expect(suggestTime("time in lon")).toBe("time in london");
    expect(suggestTime("Time in DUB")).toBe("Time in DUBAI");
    expect(suggestTime("Time in Dub")).toBe("Time in Dubai");
    expect(suggestTime("time in Afgh")).toBe("time in Afghanistan");
  });
  it("starts from a single letter and keeps going while what is typed fits", () => {
    expect(suggestTime("time in n")).toBe("time in new york");
    expect(suggestTime("time in N")).toBe("time in New York");
    expect(suggestTime("time in ne")).toBe("time in new york");
    expect(suggestTime("time in new")).toBe("time in new york");
    expect(suggestTime("time in new y")).toBe("time in new york");
    expect(suggestTime("time in New Y")).toBe("time in New York");
    expect(suggestTime("time in lo")).toBe("time in london");
    expect(suggestTime("time in LO")).toBe("time in LONDON");
    expect(suggestTime("time in Lo")).toBe("time in London");
  });
  it("suggests nothing for anything else, or once finished", () => {
    for (const q of ["tim", "chrome", "timer", "time in london", "time in new york", "time in zzzzqq", "what is the time"]) expect(suggestTime(q), q).toBeNull();
  });
});

describe("answerTime", () => {
  it("adds the difference between your zone and theirs", () => {
    const a = answerTime("what is the time in dubai", { now: winter, zone: "Asia/Calcutta" });
    expect(a.source.replace(/\s/g, " ")).toMatch(/^8:16 ?PM$/i);
    expect(a.answer.replace(/\s/g, " ")).toMatch(/^6:46 ?PM$/i);
    expect(a.toName).toMatch(/Gulf/);
    expect(a.rate).toBe("1 h 30 min behind you · Today");
  });
  it("says tomorrow and yesterday across the date line", () => {
    expect(answerTime("time in tokyo", { now: new Date("2026-01-15T20:00:00Z"), zone: "America/New_York" }).rate).toContain("Tomorrow");
    expect(answerTime("time in los angeles", { now: new Date("2026-01-15T03:00:00Z"), zone: "Asia/Kolkata" }).rate).toContain("Yesterday");
  });
  it("follows daylight saving in both zones", () => {
    expect(answerTime("time in london", { now: summer, zone: "America/New_York" }).rate).toBe("5 h ahead of you · Today");
    expect(answerTime("time in london", { now: winter, zone: "America/New_York" }).rate).toBe("5 h ahead of you · Today");
    expect(answerTime("time in london", { now: new Date("2026-03-20T12:00:00Z"), zone: "America/New_York" }).rate).toBe("4 h ahead of you · Today");
  });
  it("is the same time in the same zone", () => {
    expect(answerTime("time in delhi", { now: winter, zone: "Asia/Kolkata" }).rate).toBe("Same time as you · Today");
    expect(answerTime("what time is it", { now: winter, zone: "Asia/Kolkata" })).toBeNull();
  });
  it("is nothing for an ordinary search", () => {
    expect(answerTime("chrome")).toBeNull();
  });
});

describe("findPlace and differenceText", () => {
  it("finds places by country, city and short name", () => {
    expect(findPlace("India").zone).toBe("Asia/Kolkata");
    expect(findPlace("nowhere")).toBeNull();
  });
  it("words the difference", () => {
    expect(differenceText(330)).toBe("5 h 30 min ahead of you");
    expect(differenceText(-45)).toBe("45 min behind you");
    expect(differenceText(0)).toBe("Same time as you");
  });
});
