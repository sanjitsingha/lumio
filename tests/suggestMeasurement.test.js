import { describe, expect, it } from "vitest";
import { suggestMeasurement } from "../src/utils/units";

describe("suggestMeasurement", () => {
  it("leaves a lone number alone, for the person to add the unit", () => {
    for (const q of ["1", "2", "2 ", "12.5"]) expect(suggestMeasurement(q), q).toBeNull();
  });
  it("finishes a unit being typed", () => {
    expect(suggestMeasurement("1c")).toBe("1cm");
    expect(suggestMeasurement("1 c")).toBe("1 cm");
    expect(suggestMeasurement("5 f")).toBe("5 feet");
    expect(suggestMeasurement("5 inc")).toBe("5 inches");
    expect(suggestMeasurement("5 in")).toBe("5 inches");
    expect(suggestMeasurement("3 k")).toBe("3 km");
    expect(suggestMeasurement("3 kilog")).toBe("3 kilograms");
    expect(suggestMeasurement("10 mi")).toBe("10 miles");
    expect(suggestMeasurement("10 gall")).toBe("10 gallons");
  });
  it("follows the case being typed", () => {
    expect(suggestMeasurement("5 F")).toBe("5 Feet");
    expect(suggestMeasurement("5 FE")).toBe("5 FEET");
  });
  it("suggests where a unit goes once it is whole", () => {
    expect(suggestMeasurement("1 inch")).toBe("1 inch to cm");
    expect(suggestMeasurement("5 ft")).toBe("5 ft to cm");
    expect(suggestMeasurement("5 cm")).toBe("5 cm to inches");
    expect(suggestMeasurement("2 kg")).toBe("2 kg to pounds");
    expect(suggestMeasurement("100 lb")).toBe("100 lb to kg");
    expect(suggestMeasurement("1 mile")).toBe("1 mile to km");
  });
  it("goes on through ' to ' and the unit after it", () => {
    expect(suggestMeasurement("1 inch ")).toBe("1 inch to cm");
    expect(suggestMeasurement("1 inch t")).toBe("1 inch to cm");
    expect(suggestMeasurement("1 inch to")).toBe("1 inch to cm");
    expect(suggestMeasurement("1 inch to ")).toBe("1 inch to cm");
    expect(suggestMeasurement("1 inch to m")).toBe("1 inch to meters");
    expect(suggestMeasurement("1 inch to f")).toBe("1 inch to feet");
    expect(suggestMeasurement("1 kg to p")).toBe("1 kg to pounds");
  });
  it("stays out of the way of everything else", () => {
    for (const q of ["", "chrome", "5 star hotels", "8 gb ram", "10 minute timer", "1 inch to cm", "1 inch to feet", "2 + 2", "5 zzzz", "time"]) {
      expect(suggestMeasurement(q), q).toBeNull();
    }
  });
});
