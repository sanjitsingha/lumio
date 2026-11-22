// Spotlight's measurement answers: length, weight, temperature, area, volume,
// speed, time, data, energy, power, pressure and fuel use. Like the currency
// answers it reads how people really ask ("5'10 in cm", "1 kilomter to mile",
// "dedh kilo kitna gram") and leaves ordinary searches ("5 star hotels",
// "8 gb ram", "10 minute timer") alone. Everything is computed on this computer.

import { editDistance, MAGNITUDES, NUMBER_WORDS, readNumber } from "./currency";

const weak = { clear: false };
const u = (id, category, factor, short, name, plural, words, to, options = {}) => ({
  id, category, factor, short, name, plural, words, to, clear: true, ...options,
});
const celsius = { toBase: (v) => v, fromBase: (v) => v };
const fahrenheit = { toBase: (v) => ((v - 32) * 5) / 9, fromBase: (v) => (v * 9) / 5 + 32 };
const kelvin = { toBase: (v) => v - 273.15, fromBase: (v) => v + 273.15 };
const litresPer100 = { toBase: (v) => 100 / v, fromBase: (v) => 100 / v };
const YEAR = 365 * 86400;

/** `factor` converts to the category's base unit; `to` is where a lone amount converts. */
const UNITS = [
  // length, in metres
  u("cm", "Length", 0.01, "cm", "Centimeter", "Centimeters", ["cm", "cms", "centimeter", "centimeters", "centimetre", "centimetres", "centimter", "sentimeter"], "in"),
  u("m", "Length", 1, "m", "Meter", "Meters", ["m", "mtr", "mtrs", "meter", "meters", "metre", "metres", "miter"], "ft", { shortWeak: true }),
  u("km", "Length", 1000, "km", "Kilometer", "Kilometers", ["km", "kms", "kilometer", "kilometers", "kilometre", "kilometres", "kilomter", "kilomiter"], "mi"),
  u("mm", "Length", 0.001, "mm", "Millimeter", "Millimeters", ["mm", "mms", "millimeter", "millimeters", "millimetre", "millimetres", "milimeter", "milimeters", "milimetre"], "in"),
  u("in", "Length", 0.0254, "in", "Inch", "Inches", ["inch", "inches", "inchs", "inche", "inchh"], "cm"),
  u("ft", "Length", 0.3048, "ft", "Foot", "Feet", ["ft", "fts", "foot", "feet", "foots", "feets", "feeet"], "cm"),
  u("yd", "Length", 0.9144, "yd", "Yard", "Yards", ["yd", "yds", "yard", "yards"], "m", weak),
  u("mi", "Length", 1609.344, "mi", "Mile", "Miles", ["mi", "mile", "miles", "mil"], "km"),
  u("nmi", "Length", 1852, "nmi", "Nautical Mile", "Nautical Miles", ["nmi", "nauticalmile", "nauticalmiles"], "km"),
  u("um", "Length", 1e-6, "µm", "Micrometer", "Micrometers", ["um", "µm", "micrometer", "micrometers", "micrometre", "micron", "microns"], "mm"),
  u("nm", "Length", 1e-9, "nm", "Nanometer", "Nanometers", ["nm", "nanometer", "nanometers", "nanometre", "nanometres"], "um"),
  // weight, in kilograms
  u("kg", "Weight", 1, "kg", "Kilogram", "Kilograms", ["kg", "kgs", "kilo", "kilos", "kilogram", "kilograms", "kilogramme", "kilogrames", "kilogrm"], "lb"),
  u("g", "Weight", 0.001, "g", "Gram", "Grams", ["g", "gm", "gms", "gram", "grams", "gramme", "grammes", "gramm", "grms"], "oz", { shortWeak: true }),
  u("mg", "Weight", 1e-6, "mg", "Milligram", "Milligrams", ["mg", "mgs", "milligram", "milligrams", "miligram", "miligrams"], "g"),
  u("ug", "Weight", 1e-9, "µg", "Microgram", "Micrograms", ["ug", "µg", "mcg", "microgram", "micrograms"], "mg"),
  u("t", "Weight", 1000, "t", "Metric Ton", "Metric Tons", ["t", "ton", "tons", "tonne", "tonnes", "metricton", "metrictons", "metrictonne"], "kg", { shortWeak: true }),
  u("quintal", "Weight", 100, "q", "Quintal", "Quintals", ["quintal", "quintals", "qtl", "kuintal", "kwintal"], "kg"),
  u("lb", "Weight", 0.45359237, "lb", "Pound", "Pounds", ["lb", "lbs", "pound", "pounds"], "kg"),
  u("oz", "Weight", 0.028349523125, "oz", "Ounce", "Ounces", ["oz", "ozs", "ounce", "ounces"], "g"),
  u("stone", "Weight", 6.35029318, "st", "Stone", "Stone", ["stone", "stones"], "kg", weak),
  u("tola", "Weight", 0.0116638125, "tola", "Tola", "Tolas", ["tola", "tolas", "tolla", "tole"], "g"),
  u("carat", "Weight", 0.0002, "ct", "Carat", "Carats", ["carat", "carats"], "g"),
  u("shortton", "Weight", 907.18474, "US ton", "US Ton", "US Tons", ["shortton", "shorttons", "uston", "ustons"], "kg"),
  u("longton", "Weight", 1016.0469088, "UK ton", "UK Ton", "UK Tons", ["longton", "longtons", "ukton", "uktons"], "kg"),
  // temperature, in Celsius
  u("c", "Temperature", 1, "°C", "Celsius", "Celsius", ["c", "celsius", "celcius", "celsious", "selsius", "centigrade", "degc"], "f", { shortWeak: true, ...celsius }),
  u("f", "Temperature", 1, "°F", "Fahrenheit", "Fahrenheit", ["f", "fahrenheit", "farenheit", "fahrenhiet", "farhenheit", "ferenheit", "farenhite", "fahrenhite", "degf"], "c", { shortWeak: true, ...fahrenheit }),
  u("k", "Temperature", 1, "K", "Kelvin", "Kelvin", ["k", "kelvin", "kelvins"], "c", { shortWeak: true, ...kelvin }),
  // area, in square metres
  u("sqm", "Area", 1, "m²", "Square Meter", "Square Meters", ["sqm", "sqmt", "sqmtr", "sqmeter", "sqmeters", "sqmetre", "m2"], "sqft"),
  u("sqft", "Area", 0.09290304, "ft²", "Square Foot", "Square Feet", ["sqft", "sqfeet", "sqfoot", "squarefeet", "squarefoot", "ft2"], "sqm"),
  u("sqyd", "Area", 0.83612736, "yd²", "Square Yard", "Square Yards", ["sqyd", "sqyard", "sqyards", "yd2", "gaj", "gaz", "sqgaj"], "sqft"),
  u("sqin", "Area", 0.00064516, "in²", "Square Inch", "Square Inches", ["sqin", "sqinch", "in2"], "sqcm"),
  u("sqcm", "Area", 0.0001, "cm²", "Square Centimeter", "Square Centimeters", ["sqcm", "cm2"], "sqin"),
  u("sqmm", "Area", 1e-6, "mm²", "Square Millimeter", "Square Millimeters", ["sqmm", "mm2"], "sqcm"),
  u("sqkm", "Area", 1e6, "km²", "Square Kilometer", "Square Kilometers", ["sqkm", "km2"], "sqmi"),
  u("sqmi", "Area", 2589988.110336, "mi²", "Square Mile", "Square Miles", ["sqmi", "sqmile", "sqmiles", "mi2"], "sqkm"),
  u("acre", "Area", 4046.8564224, "ac", "Acre", "Acres", ["acre", "acres", "akre", "ekar"], "ha"),
  u("ha", "Area", 10000, "ha", "Hectare", "Hectares", ["ha", "hectare", "hectares", "hecter", "hactare", "hectar"], "acre"),
  u("guntha", "Area", 101.17141056, "guntha", "Guntha", "Gunthas", ["guntha", "gunthas", "gunta", "guntas"], "sqft"),
  // volume, in litres
  u("l", "Volume", 1, "L", "Liter", "Liters", ["l", "ltr", "ltrs", "liter", "liters", "litre", "litres", "litr"], "gal", { shortWeak: true }),
  u("ml", "Volume", 0.001, "mL", "Milliliter", "Milliliters", ["ml", "mls", "milliliter", "milliliters", "millilitre", "millilitres", "mililiter", "mililitre"], "floz"),
  u("gal", "Volume", 3.785411784, "gal", "US Gallon", "US Gallons", ["gal", "gals", "gallon", "gallons", "galon", "galons", "usgallon", "usgallons"], "l"),
  u("ukgal", "Volume", 4.54609, "UK gal", "UK Gallon", "UK Gallons", ["ukgal", "ukgallon", "ukgallons", "imperialgallon", "imperialgallons"], "l"),
  u("floz", "Volume", 0.0295735295625, "fl oz", "Fluid Ounce", "Fluid Ounces", ["floz", "fluidounce", "fluidounces", "fluidoz"], "ml"),
  u("cup", "Volume", 0.2365882365, "cup", "US Cup", "US Cups", ["cup", "cups"], "ml", weak),
  u("pint", "Volume", 0.473176473, "pt", "US Pint", "US Pints", ["pint", "pints"], "ml", weak),
  u("quart", "Volume", 0.946352946, "qt", "US Quart", "US Quarts", ["quart", "quarts"], "l", weak),
  u("tbsp", "Volume", 0.01478676478125, "tbsp", "Tablespoon", "Tablespoons", ["tbsp", "tbsps", "tablespoon", "tablespoons"], "ml"),
  u("tsp", "Volume", 0.00492892159375, "tsp", "Teaspoon", "Teaspoons", ["tsp", "tsps", "teaspoon", "teaspoons"], "ml"),
  u("m3", "Volume", 1000, "m³", "Cubic Meter", "Cubic Meters", ["m3"], "l"),
  u("cm3", "Volume", 0.001, "cm³", "Cubic Centimeter", "Cubic Centimeters", ["cm3", "cc"], "ml"),
  u("ft3", "Volume", 28.316846592, "ft³", "Cubic Foot", "Cubic Feet", ["ft3"], "l"),
  u("in3", "Volume", 0.016387064, "in³", "Cubic Inch", "Cubic Inches", ["in3"], "ml"),
  // speed, in metres per second
  u("kmh", "Speed", 1 / 3.6, "km/h", "Kilometer per Hour", "Kilometers per Hour", ["kmh", "kmph", "kph", "kmhr"], "mph"),
  u("mph", "Speed", 0.44704, "mph", "Mile per Hour", "Miles per Hour", ["mph"], "kmh"),
  u("mps", "Speed", 1, "m/s", "Meter per Second", "Meters per Second", ["mps"], "kmh"),
  u("fps", "Speed", 0.3048, "ft/s", "Foot per Second", "Feet per Second", ["fps"], "mps", weak),
  u("knot", "Speed", 1852 / 3600, "kn", "Knot", "Knots", ["knot", "knots"], "kmh", weak),
  // time, in seconds
  u("ms", "Time", 0.001, "ms", "Millisecond", "Milliseconds", ["ms", "millisecond", "milliseconds", "milisecond", "miliseconds"], "s", weak),
  u("s", "Time", 1, "s", "Second", "Seconds", ["s", "sec", "secs", "second", "seconds"], "min", weak),
  u("min", "Time", 60, "min", "Minute", "Minutes", ["min", "mins", "minute", "minutes", "minit", "minut"], "h", weak),
  u("h", "Time", 3600, "h", "Hour", "Hours", ["h", "hr", "hrs", "hour", "hours", "ghanta", "ghante"], "min", weak),
  u("day", "Time", 86400, "day", "Day", "Days", ["d", "day", "days", "din"], "h", weak),
  u("week", "Time", 604800, "week", "Week", "Weeks", ["wk", "wks", "week", "weeks", "hafta", "hafte"], "day", weak),
  u("month", "Time", YEAR / 12, "month", "Month", "Months", ["month", "months", "mahina", "mahine"], "day", weak),
  u("year", "Time", YEAR, "year", "Year", "Years", ["yr", "yrs", "year", "years", "saal"], "day", weak),
  u("decade", "Time", YEAR * 10, "decade", "Decade", "Decades", ["decade", "decades"], "year", weak),
  u("century", "Time", YEAR * 100, "century", "Century", "Centuries", ["century", "centuries"], "year", weak),
  // data, in bytes (1 GB = 1000 MB, 1 GiB = 1024 MiB)
  u("byte", "Data", 1, "B", "Byte", "Bytes", ["b", "byte", "bytes"], "kb", weak),
  u("bit", "Data", 0.125, "bit", "Bit", "Bits", ["bit", "bits"], "byte", weak),
  u("kb", "Data", 1e3, "KB", "Kilobyte", "Kilobytes", ["kb", "kilobyte", "kilobytes"], "mb", weak),
  u("mb", "Data", 1e6, "MB", "Megabyte", "Megabytes", ["mb", "megabyte", "megabytes"], "gb", weak),
  u("gb", "Data", 1e9, "GB", "Gigabyte", "Gigabytes", ["gb", "gigabyte", "gigabytes"], "mb", weak),
  u("tb", "Data", 1e12, "TB", "Terabyte", "Terabytes", ["tb", "terabyte", "terabytes"], "gb", weak),
  u("pb", "Data", 1e15, "PB", "Petabyte", "Petabytes", ["pb", "petabyte", "petabytes"], "tb", weak),
  u("kib", "Data", 1024, "KiB", "Kibibyte", "Kibibytes", ["kib", "kibibyte", "kibibytes"], "kb"),
  u("mib", "Data", 1024 ** 2, "MiB", "Mebibyte", "Mebibytes", ["mib", "mebibyte", "mebibytes"], "mb"),
  u("gib", "Data", 1024 ** 3, "GiB", "Gibibyte", "Gibibytes", ["gib", "gibibyte", "gibibytes"], "gb"),
  u("tib", "Data", 1024 ** 4, "TiB", "Tebibyte", "Tebibytes", ["tib", "tebibyte", "tebibytes"], "tb"),
  // data speed, in bits per second
  u("bps", "Data Speed", 1, "bps", "Bit per Second", "Bits per Second", ["bps"], "kbps", weak),
  u("kbps", "Data Speed", 1e3, "Kbps", "Kilobit per Second", "Kilobits per Second", ["kbps"], "mbps"),
  u("mbps", "Data Speed", 1e6, "Mbps", "Megabit per Second", "Megabits per Second", ["mbps", "mbit", "mbits"], "MBs"),
  u("gbps", "Data Speed", 1e9, "Gbps", "Gigabit per Second", "Gigabits per Second", ["gbps", "gbit", "gbits"], "mbps"),
  u("kBs", "Data Speed", 8e3, "KB/s", "Kilobyte per Second", "Kilobytes per Second", [], "MBs"),
  u("MBs", "Data Speed", 8e6, "MB/s", "Megabyte per Second", "Megabytes per Second", [], "mbps"),
  u("GBs", "Data Speed", 8e9, "GB/s", "Gigabyte per Second", "Gigabytes per Second", [], "gbps"),
  // energy, in joules
  u("j", "Energy", 1, "J", "Joule", "Joules", ["j", "joule", "joules"], "cal", weak),
  u("kj", "Energy", 1000, "kJ", "Kilojoule", "Kilojoules", ["kj", "kilojoule", "kilojoules"], "kcal"),
  u("cal", "Energy", 4.184, "cal", "Calorie", "Calories", ["cal", "cals", "calorie", "calories"], "j", weak),
  u("kcal", "Energy", 4184, "kcal", "Kilocalorie", "Kilocalories", ["kcal", "kcals", "kilocalorie", "kilocalories"], "kj"),
  u("wh", "Energy", 3600, "Wh", "Watt Hour", "Watt Hours", ["wh", "watthour", "watthours"], "kwh"),
  u("kwh", "Energy", 3.6e6, "kWh", "Kilowatt Hour", "Kilowatt Hours", ["kwh", "kilowatthour", "kilowatthours"], "kj"),
  u("btu", "Energy", 1055.05585262, "BTU", "BTU", "BTU", ["btu", "btus"], "kj"),
  // power, in watts
  u("w", "Power", 1, "W", "Watt", "Watts", ["w", "watt", "watts"], "kw", weak),
  u("kw", "Power", 1000, "kW", "Kilowatt", "Kilowatts", ["kw", "kilowatt", "kilowatts"], "hp"),
  u("mw", "Power", 1e6, "MW", "Megawatt", "Megawatts", ["mw", "megawatt", "megawatts"], "kw"),
  u("hp", "Power", 745.69987158227, "hp", "Horsepower", "Horsepower", ["hp", "bhp", "horsepower"], "kw", weak),
  // pressure, in pascals
  u("pa", "Pressure", 1, "Pa", "Pascal", "Pascals", ["pa", "pascal", "pascals"], "kpa", weak),
  u("kpa", "Pressure", 1000, "kPa", "Kilopascal", "Kilopascals", ["kpa", "kilopascal", "kilopascals"], "psi"),
  u("bar", "Pressure", 1e5, "bar", "Bar", "Bar", ["bar", "bars"], "psi", weak),
  u("psi", "Pressure", 6894.757293168, "psi", "PSI", "PSI", ["psi"], "bar"),
  u("atm", "Pressure", 101325, "atm", "Atmosphere", "Atmospheres", ["atm", "atmosphere", "atmospheres"], "kpa"),
  u("mmhg", "Pressure", 133.322387415, "mmHg", "mmHg", "mmHg", ["mmhg", "torr"], "kpa"),
  // fuel use, in kilometres per litre
  u("kmpl", "Fuel", 1, "km/L", "Kilometer per Liter", "Kilometers per Liter", ["kmpl", "kml"], "mpg"),
  u("mpg", "Fuel", 0.425143707, "mpg", "Mile per US Gallon", "Miles per US Gallon", ["mpg"], "kmpl"),
  u("l100", "Fuel", 1, "L/100km", "Liter per 100 km", "Liters per 100 km", [], "kmpl", litresPer100),
];

/** Short names that change with the amount: "1 day", "2 days". */
const MANY = { day: "days", week: "weeks", month: "months", year: "years", decade: "decades", century: "centuries", cup: "cups", tola: "tolas", guntha: "gunthas" };
const shortFor = (unit, value) => (value === 1 ? unit.short : (MANY[unit.id] ?? unit.short));

const BY_ID = new Map(UNITS.map((unit) => [unit.id, unit]));
const UNIT_OF = new Map();
for (const unit of UNITS) for (const word of unit.words) if (!UNIT_OF.has(word)) UNIT_OF.set(word, unit);

/** "sq ft", "cubic meter". */
const SQUARE_OF = { mm: "sqmm", cm: "sqcm", m: "sqm", km: "sqkm", in: "sqin", ft: "sqft", yd: "sqyd", mi: "sqmi" };
const CUBIC_OF = { cm: "cm3", m: "m3", ft: "ft3", in: "in3" };
/** "km per hour", "m/s", "km/l". */
const PER = {
  "km/h": "kmh", "km/min": "kmh", "mi/h": "mph", "m/s": "mps", "ft/s": "fps", "km/l": "kmpl", "mi/gal": "mpg",
  "kb/s": "kBs", "mb/s": "MBs", "gb/s": "GBs",
};
const SQUARE_WORDS = new Set(["sq", "sqr", "square", "sqare", "sqaure", "squre", "squar", "squared"]);
const CUBIC_WORDS = new Set(["cu", "cubic", "cube", "cubed"]);
/** Fractions and Hindi quantities: "half kg", "dedh kilo", "dhai feet". */
const FRACTIONS = new Map(
  Object.entries({ half: 0.5, aadha: 0.5, adha: 0.5, aadhi: 0.5, adhi: 0.5, quarter: 0.25, pav: 0.25, paav: 0.25, dedh: 1.5, derh: 1.5, dhai: 2.5, dhaai: 2.5, sawa: 1.25 }),
);
const VULGAR = { "½": ".5", "¼": ".25", "¾": ".75", "⅓": ".3333333333", "⅔": ".6666666667" };
const DIRECTION_WORDS = new Set(["to", "in", "into", "me", "mein", "main", "mai", "ka", "ki", "ke", "se", "में", "का", "की", "के", "से"]);
/** Words that tie two units into a question ("1 kg kitna gram", "1 km equals how many m"). */
const LINK_WORDS = new Set([...DIRECTION_WORDS, "per", "of", "is", "are", "equal", "equals", "how", "many", "much", "make", "makes", "kitna", "kitne", "kitni", "कितना", "कितने", "कितनी"]);
const FILLER = new Set(
  `what whats wat wats wht is are was were the of for conversion convert converted converter converting conv calculate calc
  change how hw much mch many does do did it its make makes equal equals equivalent exactly approx approximately about around
  roughly now please pls plz tell give show find get check at on my from and or than would be can could will should you your
  need want know say like with this that there has have had if i we am so just only hi hello hey bro sir thanks thank value
  height length weight distance temperature temp speed area volume size tall long wide heavy far weigh weighs mass degree degrees deg
  kya hai hain hoga hota hote hoti hua hue aur wala wale mujhe bataiye batao btao bata kitna kitne kitni ko par barabar yaar bhai
  है हैं क्या को बराबर कितना कितने कितनी`.split(/\s+/),
);
/** What a misspelled word may have meant, by first letter; on a tie the common word wins ("waht" is not "watt"). */
const TYPO_TARGETS = new Map();
for (const word of [...FILLER, ...UNIT_OF.keys(), ...SQUARE_WORDS, ...CUBIC_WORDS]) {
  if (word.length < 4 || !/^[a-z]+$/.test(word)) continue;
  if (!TYPO_TARGETS.has(word[0])) TYPO_TARGETS.set(word[0], []);
  TYPO_TARGETS.get(word[0]).push(word);
}

const isUnit = (part) => part?.type === "unit";
const isNumber = (part) => part?.type === "number";
/** A letter or two ("5 m", "5 g") could be anything; the written name ("5 meters") is clear. */
const unitToken = (unit, word = "", raw = word) => ({ type: "unit", unit, raw, clear: unit.clear && !(unit.shortWeak && word.length < 3) });
const sameLetters = (a, b) => [...a].sort().join("") === [...b].sort().join("");

function closest(word) {
  let best = null;
  let bestDistance = 3;
  for (const name of TYPO_TARGETS.get(word[0]) ?? []) {
    const allowed = name.length >= 7 ? 2 : 1;
    if (Math.abs(name.length - word.length) > allowed) continue;
    // a short word is a typo of a filler only if its letters were swapped: "walk" is not "wala"
    if (word.length <= 4 && FILLER.has(name) && !sameLetters(word, name)) continue;
    const distance = editDistance(word, name);
    if (distance <= allowed && distance < bestDistance) {
      best = name;
      bestDistance = distance;
    }
  }
  return best;
}

function normalize(text) {
  return text
    .replace(/[½¼¾⅓⅔]/g, (c) => VULGAR[c])
    .replace(/(\d)\s+\.(\d)/g, (all, a, b) => `${Number(a) + Number(`.${b}`)}`.length ? `${a}.${b}` : all)
    .replace(/\bl\s*(?:\/|per)\s*100\s*kms?\b|\b(?:liters?|litres?)\s*per\s*100\s*kms?\b/gi, " §l100§ ")
    .replace(/(\d)\s*['’′]\s*(\d+(?:\.\d+)?)\s*(?:["”″]|inch(?:es)?\b|in\b)?/g, "$1 ft $2 in ")
    .replace(/(\d)\s*['’′]/g, "$1 ft ")
    .replace(/(\d)\s*["”″]/g, "$1 in ")
    .replace(/=>|->|→|=/g, " to ")
    .replace(/(\d+)\s+(\d+)\/(\d+)(?!\d)/g, (_, whole, a, b) => (Number(b) ? `${Number(whole) + Number(a) / Number(b)}` : "x"))
    .replace(/(\d+)\/(\d+)(?!\d)/g, (_, a, b) => (Number(b) ? `${Number(a) / Number(b)}` : "x"))
    .replace(/\d[\d.,]*\d/g, readNumber)
    .replace(/(^|[^\d.,])(\d{1,3})((?: \d{3})+)(?!\d)/g, (_, before, first, rest) => before + first + rest.replace(/ /g, ""))
    .replace(/°\s*c\b/gi, " celsius ")
    .replace(/°\s*f\b/gi, " fahrenheit ")
    .replace(/°\s*k\b/gi, " kelvin ")
    .replace(/°/g, " ")
    .replace(/²/g, "2")
    .replace(/³/g, "3")
    .replace(/['’]/g, "")
    .replace(/\bnautical\s+miles?\b/gi, "nauticalmile")
    .replace(/\bmetric\s+tons?\b|\bmetric\s+tonnes?\b/gi, "metricton")
    .replace(/\b(?:short|us)\s+tons?\b/gi, "shortton")
    .replace(/\b(?:long|uk)\s+tons?\b/gi, "longton")
    .replace(/\b(?:uk|imperial)\s+gall?ons?\b/gi, "ukgallon")
    .replace(/\bus\s+gall?ons?\b/gi, "gallon")
    .replace(/\bfl(?:uid)?\.?\s*(?:oz|ounces?)\b/gi, "floz")
    .replace(/\bmiles?\s+per\s+gall?on\b/gi, "mpg")
    .replace(/\bkilowatt\s+hours?\b|\bkw\s+h\b/gi, "kwh")
    .replace(/\bwatt\s+hours?\b/gi, "wh")
    .replace(/\bhorse\s+power\b/gi, "horsepower")
    .replace(/\b([a-z]{1,8})2([a-z]{1,8})\b/gi, (all, a, b) =>
      UNIT_OF.has(a.toLowerCase()) && (UNIT_OF.has(b.toLowerCase()) || b.toLowerCase() === "in") ? `${a} to ${b}` : all,
    )
    .replace(/\//g, " per ")
    .replace(/\.(?!\d)/g, " ")
    .replace(/[?!,:;"“”()[\]{}\\|_~…–—&>-]/g, " ");
}

function exactToken(word, raw) {
  if (raw === "MBps") return unitToken(BY_ID.get("MBs"), raw);
  if (raw === "GBps") return unitToken(BY_ID.get("GBs"), raw);
  if (raw === "KBps") return unitToken(BY_ID.get("kBs"), raw);
  if (word === "in") return { type: "word", word, maybeInch: true };
  if (word === "per") return { type: "per", word };
  if (word === "a" || word === "an") return { type: "article" };
  if (SQUARE_WORDS.has(word)) return { type: "square" };
  if (CUBIC_WORDS.has(word)) return { type: "cubic" };
  if (FRACTIONS.has(word)) return { type: "number", value: FRACTIONS.get(word), spelled: true, fraction: true };
  if (NUMBER_WORDS.has(word)) return { type: "number", value: NUMBER_WORDS.get(word), spelled: true };
  if (DIRECTION_WORDS.has(word) || FILLER.has(word)) return { type: "word", word };
  const unit = UNIT_OF.get(word);
  if (unit) return unitToken(unit, word, raw);
  if (word.length > 1 && MAGNITUDES.has(word)) return { type: "magnitude", value: MAGNITUDES.get(word) };
  return null;
}

/** "kmtomiles", "cmtoinch". */
function splitGlued(word) {
  if (word.length < 5) return null;
  for (let i = 1; i <= word.length - 3; i++) {
    const left = UNIT_OF.get(word.slice(0, i));
    if (!left) continue;
    const rest = word.slice(i);
    for (const link of ["to", "in"]) {
      if (!rest.startsWith(link)) continue;
      const tail = rest.slice(link.length);
      const right = UNIT_OF.get(tail) ?? (tail === "in" ? BY_ID.get("in") : null);
      if (right) return [unitToken(left, word.slice(0, i)), { type: "word", word: link === "in" ? "into" : link }, unitToken(right, tail)];
    }
  }
  return null;
}

function wordToken(raw) {
  const word = raw.toLowerCase();
  const exact = exactToken(word, raw);
  if (exact) return exact;
  const glued = splitGlued(word);
  if (glued) return { type: "glued", tokens: glued };
  const near = word.length >= 4 ? closest(word) : null;
  if (near) {
    const token = exactToken(near, near);
    if (isUnit(token)) return { ...token, clear: token.clear && near.length >= 6 };
    if (token) return token;
  }
  return { type: "unknown", word };
}

/** The last word while it is still being typed: a possible start of a unit is ignored. */
function isUnfinished(word, tokens) {
  if (!tokens.some(isUnit)) return false;
  if (DIRECTION_WORDS.has(tokens.at(-1)?.word) && word.length >= 2) return false;
  return ["to", "in", "into"].some((w) => w.startsWith(word)) || [...UNIT_OF.keys()].some((name) => name.startsWith(word));
}

/** After "to", the start of a unit already names it: "5 ft to cen". */
function completeUnit(word, tokens) {
  if (!DIRECTION_WORDS.has(tokens.at(-1)?.word) || word.length < 2 || !tokens.some(isUnit)) return null;
  const from = tokens.find(isUnit).unit;
  const names = [...UNIT_OF.keys()].filter((name) => name.startsWith(word));
  const name = names.find((n) => UNIT_OF.get(n).category === from.category);
  return name ? unitToken(UNIT_OF.get(name), name) : null;
}

function scan(text) {
  const tokens = [];
  let rest = text;
  while (rest.length) {
    let m;
    if ((m = /^\s+/.exec(rest))) {
      // spaces separate tokens and mean nothing else
    } else if ((m = /^§(\w+)§/.exec(rest))) {
      tokens.push(unitToken(BY_ID.get(m[1]), m[1]));
    } else if ((m = /^(\d+(?:\.\d+)?|\.\d+)/.exec(rest))) {
      tokens.push({ type: "number", value: Number(m[1]) });
    } else if ((m = /^[\p{L}\p{M}µ]+[23]?/u.exec(rest))) {
      const atEnd = m[0].length === rest.length;
      // "5 ft t" is on its way to "to", not a ton
      if (atEnd && m[0].toLowerCase() === "t" && isUnit(tokens.at(-1))) break;
      const token = wordToken(m[0]);
      if (token.type === "glued") {
        tokens.push(...token.tokens);
      } else if (token.type === "unknown" && atEnd) {
        const completed = completeUnit(token.word, tokens);
        if (completed) tokens.push(completed);
        else if (!isUnfinished(token.word, tokens)) tokens.push(token);
      } else {
        tokens.push(token);
      }
    } else {
      return null;
    }
    rest = rest.slice(m[0].length);
  }
  return tokens;
}

/** "two hundred fifty", "1.5 lakh", "one and a half", "5k km". */
function joinNumbers(tokens) {
  const out = [];
  for (let i = 0; i < tokens.length; ) {
    const token = tokens[i];
    if (!isNumber(token) && token.type !== "magnitude") {
      out.push(tokens[i++]);
      continue;
    }
    let total = 0;
    let current = null;
    let joinable = false;
    for (; i < tokens.length; i++) {
      const next = tokens[i];
      if (next.type === "magnitude") {
        if (next.value === 100) current = (current ?? 1) * 100;
        else {
          total += (current ?? 1) * next.value;
          current = null;
        }
        joinable = true;
      } else if (isNumber(next)) {
        if (current === null) current = next.value;
        else if (joinable) current += next.value;
        else break;
        joinable = Boolean(next.spelled);
      } else if (next.word === "and" && current !== null) {
        // "one and a half"
        const j = tokens[i + 1]?.type === "article" ? i + 2 : i + 1;
        if (!tokens[j]?.fraction) break;
        current += tokens[j].value;
        i = j;
      } else break;
    }
    let value = total + (current ?? 0);
    // "5k km": a k before another unit is a thousand, not Kelvin
    if (tokens[i]?.unit?.id === "k" && isUnit(tokens[i + 1])) {
      value *= 1000;
      i++;
    }
    out.push({ type: "number", value });
  }
  return out;
}

/** "sq ft", "cubic meter", "km per hour", and "in" as an inch where it cannot mean "in". */
function joinUnits(tokens) {
  const out = [];
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    const next = tokens[i + 1];
    if (token.type === "square" || token.type === "cubic") {
      const length = next?.maybeInch ? "in" : next?.unit?.id;
      const id = (token.type === "square" ? SQUARE_OF : CUBIC_OF)[length];
      if (id) {
        out.push(unitToken(BY_ID.get(id), id));
        i++;
        continue;
      }
      if (isUnit(out.at(-1)) && token.type === "square" && SQUARE_OF[out.at(-1).unit.id]) {
        const id = SQUARE_OF[out.at(-1).unit.id]; // "feet square"
        out[out.length - 1] = unitToken(BY_ID.get(id), id);
        continue;
      }
      out.push({ type: "unknown" });
      continue;
    }
    if (token.type === "per") {
      const before = out.at(-1);
      const key = isUnit(before) && isUnit(next) ? `${before.unit.id}/${next.unit.id}` : null;
      if (key && PER[key]) {
        const id = before.raw === "Mb" ? "mbps" : before.raw === "Gb" ? "gbps" : PER[key];
        out[out.length - 1] = unitToken(BY_ID.get(id), id);
        i++;
        continue;
      }
      out.push({ type: "word", word: "per" });
      continue;
    }
    if (token.maybeInch) {
      const previous = tokens[i - 1];
      const afterNumber = isNumber(previous) && (!next || DIRECTION_WORDS.has(next.word) || isNumber(next));
      const asTarget = DIRECTION_WORDS.has(previous?.word) && !next;
      const startsPair = i === 0 && DIRECTION_WORDS.has(next?.word);
      if ((afterNumber && !isNumber(next)) || asTarget || startsPair) {
        // "1 in" alone is more likely the start of "1 in 5"
        out.push({ ...unitToken(BY_ID.get("in"), "in"), clear: false });
        continue;
      }
    }
    out.push(token);
  }
  return out;
}

/** "a kilo" is one; "do kg" is two in Hindi. */
function readArticles(tokens) {
  return tokens.flatMap((token, i) => {
    const next = tokens[i + 1];
    // "half a kg" already has its number
    if (token.type === "article") return isUnit(next) && !isNumber(tokens[i - 1]) ? [{ type: "number", value: 1 }] : [];
    if (token.word === "do" && isUnit(next)) return [{ type: "number", value: 2 }];
    return [token];
  });
}

/** "5 ft 10 in", "2 hours 30 min", "5 ft 10" become one amount in the first unit. */
function joinCompound(parts) {
  const out = [];
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i];
    const unit = parts[i + 1]?.unit;
    if (!isNumber(part) || !unit || unit.toBase) {
      out.push(part);
      continue;
    }
    let total = part.value * unit.factor;
    const pieces = [{ value: part.value, unit }];
    let j = i + 2;
    let smallest = unit;
    while (isNumber(parts[j]) && parts[j + 1]?.unit?.category === unit.category && parts[j + 1].unit.factor < smallest.factor) {
      smallest = parts[j + 1].unit;
      total += parts[j].value * smallest.factor;
      pieces.push({ value: parts[j].value, unit: smallest });
      j += 2;
    }
    // "5 ft 10" is five feet ten inches
    if (pieces.length === 1 && unit.id === "ft" && isNumber(parts[j]) && !isUnit(parts[j + 1])) {
      const inch = BY_ID.get("in");
      total += parts[j].value * inch.factor;
      pieces.push({ value: parts[j].value, unit: inch });
      j += 1;
    }
    out.push({ type: "number", value: total / unit.factor, pieces: pieces.length > 1 ? pieces : null }, parts[i + 1]);
    i = j - 1;
  }
  return out;
}

/** A gaj is a plot's square yard, but next to feet or meters it is a plain yard. */
function plainGaj(token, other) {
  const isGaj = ["gaj", "gaz"].includes(token.raw?.toLowerCase());
  return isGaj && other.unit.category === "Length" ? BY_ID.get("yd") : token.unit;
}

/** Two units that can convert; ounces of a liquid are fluid ounces ("8 oz to ml"). */
function sameCategory(firstToken, secondToken) {
  const first = plainGaj(firstToken, secondToken);
  const second = plainGaj(secondToken, firstToken);
  if (first.category === second.category) return [first, second];
  if (first.id === "oz" && second.category === "Volume") return [BY_ID.get("floz"), second];
  if (second.id === "oz" && first.category === "Volume") return [first, BY_ID.get("floz")];
  return null;
}

function sourceIndex(parts, first, second) {
  const n = parts.findIndex(isNumber);
  if (n < 0) {
    const between = parts.slice(parts.indexOf(first) + 1, parts.indexOf(second));
    return between.some((p) => p.word === "of" || p.word === "per") ? 1 : 0;
  }
  const previous = parts[n - 1];
  const next = parts[n + 1];
  if (next === first) return 0;
  if (next === second) return previous === first ? 0 : 1;
  if (previous === second && n > parts.indexOf(second)) return 0;
  return previous === second ? 1 : 0;
}

/**
 * `{ amount, from, to, pieces }` (unit ids) if `text` asks for a measurement
 * conversion, else null. A word it cannot place inside the conversion, or two
 * units that cannot convert (kg to litres), means no answer.
 */
export function parseMeasurement(text) {
  let body = text.trim();
  if (!body) return null;
  const negative = /(^|\s)(?:[-−]|minus\s+)(?=[\d.])/i;
  const sign = negative.test(body) ? -1 : 1;
  body = body.replace(negative, "$1");
  const tokens = scan(normalize(body));
  if (!tokens) return null;
  const parts = joinCompound(readArticles(joinUnits(joinNumbers(tokens))));

  const units = parts.filter(isUnit);
  const numbers = parts.filter(isNumber);
  if (units.length < 1 || units.length > 2 || numbers.length > 1) return null;

  if (parts.some((p) => p.type === "unknown")) {
    // words around a clear "5 ft to cm" are fine; words inside it change what it means
    const anchors = parts.flatMap((p, i) => (isUnit(p) || isNumber(p) ? [i] : []));
    const inside = parts.slice(anchors[0], anchors.at(-1) + 1);
    if (units.length < 2 || inside.some((p) => p.type === "unknown")) return null;
  }

  const number = numbers[0];
  let from;
  let to;
  if (units.length === 1) {
    if (!number || !(units[0].clear || number.pieces)) return null;
    // the amount must belong to the unit: "5 to cm" says nothing about what the 5 is
    if (parts.indexOf(number) !== parts.indexOf(units[0]) - 1 && parts[parts.indexOf(units[0]) + 1] !== number) return null;
    from = units[0].unit;
    to = BY_ID.get(from.to);
  } else {
    const [first, second] = units;
    const between = parts.slice(parts.indexOf(first) + 1, parts.indexOf(second));
    // Hindi puts it after the unit: "aadha kilo gram me"
    const linked = between.some((p) => LINK_WORDS.has(p.word)) || DIRECTION_WORDS.has(parts[parts.indexOf(second) + 1]?.word);
    if (!linked && !(first.clear && second.clear)) return null;
    const pair = sameCategory(first, second);
    if (!pair) return null;
    const index = sourceIndex(parts, first, second);
    from = pair[index];
    to = pair[1 - index];
  }
  const amount = (number?.value ?? 1) * sign;
  if (sign < 0 && from.category !== "Temperature") return null;
  if (!Number.isFinite(amount) || Math.abs(amount) > 1e15) return null;
  return { amount, from: from.id, to: to.id, pieces: number?.pieces ?? null };
}

/** Up to seven significant digits, so that float noise never shows. */
export function formatQuantity(value) {
  if (value === 0) return "0";
  const size = Math.abs(value);
  if (size >= 1e15 || size < 1e-6) return value.toExponential(4).replace(/\.?0+e/, "e").replace("e+", "e");
  return new Intl.NumberFormat("en-US", { maximumSignificantDigits: 7 }).format(value);
}

const withUnit = (value, unit) => `${formatQuantity(value)}${unit.short.startsWith("°") ? "" : " "}${shortFor(unit, value)}`;

function convert(value, from, to) {
  const base = from.toBase ? from.toBase(value) : value * from.factor;
  return to.fromBase ? to.fromBase(base) : base / to.factor;
}

const FORMULAS = {
  "c>f": "°F = °C × 9/5 + 32",
  "f>c": "°C = (°F − 32) × 5/9",
  "c>k": "K = °C + 273.15",
  "k>c": "°C = K − 273.15",
  "f>k": "K = (°F − 32) × 5/9 + 273.15",
  "k>f": "°F = (K − 273.15) × 9/5 + 32",
};

/**
 * What Spotlight shows for `text`, or null if it is not a measurement:
 * the same shape as a currency answer, so that the same panel draws it.
 */
export function answerMeasurement(text) {
  const parsed = parseMeasurement(text);
  if (!parsed) return null;
  const from = BY_ID.get(parsed.from);
  const to = BY_ID.get(parsed.to);
  const value = convert(parsed.amount, from, to);
  if (!Number.isFinite(value)) return null;
  const source = parsed.pieces
    ? parsed.pieces.map((piece) => withUnit(piece.value, piece.unit)).join(" ")
    : withUnit(parsed.amount, from);
  const inverse = from.category === "Fuel" && (from.id === "l100" || to.id === "l100");
  return {
    measure: true,
    amount: parsed.amount,
    from: shortFor(from, parsed.amount),
    to: shortFor(to, value),
    fromName: parsed.amount === 1 ? from.name : from.plural,
    toName: value === 1 ? to.name : to.plural,
    source,
    answer: withUnit(value, to),
    rate: FORMULAS[`${from.id}>${to.id}`] ?? (inverse ? `L/100km = 100 ÷ km/L` : `1 ${from.short} = ${withUnit(convert(1, from, to), to)}`),
    updated: from.category,
    webQuery: `${source} to ${shortFor(to, value)}`,
  };
}
