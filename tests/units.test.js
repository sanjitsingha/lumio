import { describe, expect, it } from "vitest";
import { answerMeasurement, formatQuantity, parseMeasurement } from "../src/utils/units";
import { answerConversion } from "../src/utils/currency";

const pair = (text) => {
  const r = parseMeasurement(text);
  return r ? `${Number(r.amount.toPrecision(10))} ${r.from} ${r.to}` : null;
};
const answer = (text) => answerMeasurement(text)?.answer ?? null;

describe("parseMeasurement", () => {
  it.each([
    ["5 feet to cm", "5 ft cm"],
    ["5ft to cm", "5 ft cm"],
    ["5 ft in cm", "5 ft cm"],
    ["5 ft = cm", "5 ft cm"],
    ["5 ft -> cm", "5 ft cm"],
    ["5 ft", "5 ft cm"],
    ["10 km", "10 km mi"],
    ["70 kg", "70 kg lb"],
    ["what is 5 feet in cm", "5 ft cm"],
    ["convert 10 km to miles", "10 km mi"],
    ["how many cm in an inch", "1 in cm"],
    ["how many feet in a mile", "1 mi ft"],
    ["how many days in a year", "1 year day"],
    ["km to miles", "1 km mi"],
    ["kg to lbs", "1 kg lb"],
    ["c to f", "1 c f"],
    ["in to cm", "1 in cm"],
    ["cm to in", "1 cm in"],
    ["1 in to cm", "1 in cm"],
    ["10 cm in in", "10 cm in"],
    ["1 pound to kg", "1 lb kg"],
    ["5 pounds in grams", "5 lb g"],
    ["1 ton to kg", "1 t kg"],
    ["1 us ton to kg", "1 shortton kg"],
    ["kmtomiles", "1 km mi"],
    ["10kmtomiles", "10 km mi"],
    ["cm2in", "1 cm in"],
    ["hey bro 5 ft to cm thanks", "5 ft cm"],
    ["can you please tell me 10 km in miles", "10 km mi"],
  ])("basics: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["5'10", "5.833333333 ft cm"],
    ["5'10\"", "5.833333333 ft cm"],
    ["5'10 in cm", "5.833333333 ft cm"],
    ["5 ft 10 in", "5.833333333 ft cm"],
    ["5 ft 10", "5.833333333 ft cm"],
    ["5 feet 10 inches to cm", "5.833333333 ft cm"],
    ["height 5 ft 10 in cm", "5.833333333 ft cm"],
    ["2 hours 30 minutes to minutes", "2.5 h min"],
    ["2 kg 500 g to lbs", "2.5 kg lb"],
    ["1 km 500 m to miles", "1.5 km mi"],
  ])("compound: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["1 kilomter to mile", "1 km mi"],
    ["5 centimter to inch", "5 cm in"],
    ["10 milimeter to cm", "10 mm cm"],
    ["5 inchs to cm", "5 in cm"],
    ["5 feets to meter", "5 ft m"],
    ["1 kilogrm to gram", "1 kg g"],
    ["100 celcius to farenheit", "100 c f"],
    ["98.6 fahrenhiet to celsious", "98.6 f c"],
    ["1 galon to litre", "1 gal l"],
    ["waht is 5 fet to cm", null],
    ["waht is 5 feet to cm plz", "5 ft cm"],
    ["1 sqaure feet to sq meter", "1 sqft sqm"],
    ["convrt 2 hectar to acre", "2 ha acre"],
  ])("typos: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["1 kg kitna gram", "1 kg g"],
    ["1 kg me kitne gram hote hai", "1 kg g"],
    ["dedh kilo kitna gram", "1.5 kg g"],
    ["dhai feet kitna cm", "2.5 ft cm"],
    ["aadha kilo gram me", "0.5 kg g"],
    ["do kg kitna gram", "2 kg g"],
    ["ek ghanta kitne minute", "1 h min"],
    ["1 saal me kitne din", "1 year day"],
    ["100 gaj kitna sq ft", "100 sqyd sqft"],
    ["1 gaj me kitne feet", "1 yd ft"],
    ["1 tola kitna gram", "1 tola g"],
    ["1 quintal me kitna kg", "1 quintal kg"],
    ["1 acre kitna guntha", "1 acre guntha"],
  ])("Hindi and Hinglish: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["1,000 m to km", "1000 m km"],
    ["1.5 km to m", "1.5 km m"],
    ["1,5 km to m", "1.5 km m"],
    ["1 000 m to km", "1000 m km"],
    ["1/2 cup to ml", "0.5 cup ml"],
    ["1 1/2 cups to ml", "1.5 cup ml"],
    ["½ kg to g", "0.5 kg g"],
    ["half kg to g", "0.5 kg g"],
    ["half a kg to g", "0.5 kg g"],
    ["one and a half kg to g", "1.5 kg g"],
    ["two hundred fifty grams to kg", "250 g kg"],
    ["1 lakh km to miles", "100000 km mi"],
    ["5k km to miles", "5000 km mi"],
  ])("amounts: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    ["100 km/h to mph", "100 kmh mph"],
    ["100 kmph to mph", "100 kmh mph"],
    ["100 km per hour in mph", "100 kmh mph"],
    ["60 miles per hour to kmph", "60 mph kmh"],
    ["10 m/s to km/h", "10 mps kmh"],
    ["100 sq ft to sq m", "100 sqft sqm"],
    ["100 square feet to square meters", "100 sqft sqm"],
    ["10 m2 to ft2", "10 sqm sqft"],
    ["10 m² to ft²", "10 sqm sqft"],
    ["1 cubic meter to liters", "1 m3 l"],
    ["1 m3 to l", "1 m3 l"],
    ["8 oz to ml", "8 floz ml"],
    ["8 fl oz to ml", "8 floz ml"],
    ["1 uk gallon to liters", "1 ukgal l"],
    ["100 mbps to MB/s", "100 mbps MBs"],
    ["100 Mb/s to MBps", "100 mbps MBs"],
    ["1 gb to mb", "1 gb mb"],
    ["1 gib to mib", "1 gib mib"],
    ["100 hp to kw", "100 hp kw"],
    ["1 kwh to kj", "1 kwh kj"],
    ["32 psi to bar", "32 psi bar"],
    ["20 kmpl to mpg", "20 kmpl mpg"],
    ["5 l/100km to kmpl", "5 l100 kmpl"],
    ["100°c to f", "100 c f"],
    ["100 °F in celsius", "100 f c"],
    ["37°c", "37 c f"],
    ["-40 c to f", "-40 c f"],
    ["minus 10 celsius to fahrenheit", "-10 c f"],
    ["37 degree celsius to fahrenheit", "37 c f"],
  ])("kinds: %s", (text, expected) => {
    expect(pair(text)).toBe(expected);
  });

  it.each([
    "",
    "km",
    "feet",
    "5",
    "chrome",
    "5 star hotels",
    "1 hour timer",
    "10 minute timer",
    "2 days",
    "24 hours",
    "1 year",
    "5g",
    "4g phone",
    "3d",
    "2k",
    "4k video",
    "1080p",
    "8 gb ram",
    "16 gb",
    "100 days of code",
    "top 10 movies",
    "1 in 5",
    "5 in a row",
    "2 player games",
    "1 km walk",
    "10 am to 5 pm",
    "5 to cm",
    "100 in km",
    "1 kg to litre",
    "5 feet to kg",
    "1 usd to km",
    "-5 km to m",
    "hp laptop",
    "bar near me",
    "cup of tea",
    "1 cup",
    "5 cups",
    "1 pint",
    "constructor",
  ])("leaves %j alone", (text) => {
    expect(parseMeasurement(text)).toBeNull();
  });

  it("keeps answering while the next word is being typed", () => {
    expect(pair("5 ft t")).toBe("5 ft cm");
    expect(pair("5 ft to")).toBe("5 ft cm");
    expect(pair("5 ft to me")).toBe("5 ft cm");
    expect(pair("5 ft to yar")).toBe("5 ft yd");
    expect(pair("5 ft tall person")).toBeNull();
  });
});

describe("answerMeasurement", () => {
  it.each([
    ["1 inch to cm", "2.54 cm"],
    ["1 ft to cm", "30.48 cm"],
    ["1 mile to km", "1.609344 km"],
    ["100 km to miles", "62.13712 mi"],
    ["1 kg to lb", "2.204623 lb"],
    ["1 lb to g", "453.5924 g"],
    ["1 tola to g", "11.66381 g"],
    ["1 quintal to kg", "100 kg"],
    ["100 c to f", "212°F"],
    ["32 f to c", "0°C"],
    ["98.6 f to c", "37°C"],
    ["0 c to k", "273.15 K"],
    ["-40 c to f", "-40°F"],
    ["1 acre to sq ft", "43,560 ft²"],
    ["1 hectare to acre", "2.471054 ac"],
    ["1 sq yard to sq ft", "9 ft²"],
    ["1 guntha to sq ft", "1,089 ft²"],
    ["1 gallon to l", "3.785412 L"],
    ["1 cup to ml", "236.5882 mL"],
    ["100 kmph to mph", "62.13712 mph"],
    ["1 hour to minutes", "60 min"],
    ["1 year to days", "365 days"],
    ["1 week to days", "7 days"],
    ["7 days to week", "1 week"],
    ["2 cups to ml", "473.1765 mL"],
    ["1 gaj to feet", "3 ft"],
    ["1 gb to mb", "1,000 MB"],
    ["1 gib to mib", "1,024 MiB"],
    ["100 mbps to MB/s", "12.5 MB/s"],
    ["1 kwh to kj", "3,600 kJ"],
    ["1 hp to w", "745.6999 W"],
    ["1 bar to psi", "14.50377 psi"],
    ["5 l/100km to kmpl", "20 km/L"],
    ["5'10", "177.8 cm"],
  ])("%s = %s", (text, expected) => {
    expect(answer(text)).toBe(expected);
  });

  it("gives the panel both sides", () => {
    expect(answerMeasurement("5 ft to cm")).toEqual({
      measure: true,
      amount: 5,
      from: "ft",
      to: "cm",
      fromName: "Feet",
      toName: "Centimeters",
      source: "5 ft",
      answer: "152.4 cm",
      rate: "1 ft = 30.48 cm",
      updated: "Length",
      webQuery: "5 ft to cm",
    });
  });

  it("shows height as it was typed and temperature with its formula", () => {
    expect(answerMeasurement("5'10").source).toBe("5 ft 10 in");
    expect(answerMeasurement("100 c to f").rate).toBe("°F = °C × 9/5 + 32");
    expect(answerMeasurement("1 kg").fromName).toBe("Kilogram");
    expect(answerMeasurement("1 day to hours").source).toBe("1 day");
    expect(answerMeasurement("2 days to hours").source).toBe("2 days");
    expect(answerMeasurement("1 year to days").rate).toBe("1 year = 365 days");
  });

  it("is null for anything else", () => {
    expect(answerMeasurement("chrome")).toBeNull();
  });
});

describe("formatQuantity", () => {
  it("hides float noise and keeps huge and tiny values readable", () => {
    expect(formatQuantity(0.1 + 0.2)).toBe("0.3");
    expect(formatQuantity(1234567.89)).toBe("1,234,568");
    expect(formatQuantity(2.5e-9)).toBe("2.5e-9");
    expect(formatQuantity(3e20)).toBe("3e20");
  });
});

describe("currency and measurement side by side", () => {
  const rates = { base: "USD", updatedAt: 1795305600, rates: { USD: 1, INR: 92.8891, GBP: 0.8 } };

  it("keeps money for currency and weight for measurement", () => {
    expect(answerConversion("1 pound to inr", rates)?.to).toBe("INR");
    expect(answerConversion("1 pound to kg", rates)).toBeNull();
    expect(answerMeasurement("1 pound to kg")?.to).toBe("kg");
    expect(answerMeasurement("1 pound to inr")).toBeNull();
  });
});
