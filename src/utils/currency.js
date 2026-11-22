// Spotlight's currency answers: understands how people really ask ("$1",
// "1 dolar to nepali", "what is inr value of $1", "100 usd ka rupee") and
// answers from the saved rates, so an ordinary search ("1 pen", "india vs
// pakistan", "dollar shave club") is left alone.

import { CURRENCIES, LOOSE_UNITS, WORD_CODES } from "./currencyData";

/** Where a lone amount converts to; this currency itself converts to `AWAY`. */
const HOME = "INR";
const AWAY = "USD";
const MAX_AMOUNT = 1e15;

/** Other ways people write a unit. Near misses are caught by `closest`. */
const SPELLINGS = {
  dollar: ["doller", "dollers", "dolar", "dolars", "dollor", "dollors", "dallar", "dalar", "डॉलर", "डालर"],
  rupee: ["ruppee", "ruppees", "rupe", "rupes", "rupay", "rupaya", "rupaye", "rupiya", "rupiye", "rupye", "rupya", "ruppe", "rs", "रुपये", "रुपया", "रुपए", "रूपये", "रूपया"],
  euro: ["eruo", "uero", "euor", "यूरो"],
  pound: ["पाउंड"],
  yen: ["येन"],
  dirham: ["dirhum", "दिरहम"],
  riyal: ["reyal", "रियाल"],
};
/** Names safe to correct on their own; others sit one letter from real words (royal, diner). */
const CORRECTABLE = new Set(["dollar", "dollars", "rupee", "rupees", "pound", "pounds", "euro", "euros", "dirham", "dirhams", "ringgit"]);
const SUBUNITS = new Map([
  ["cent", { code: "USD", scale: 0.01 }],
  ["cents", { code: "USD", scale: 0.01 }],
  ["paisa", { code: "INR", scale: 0.01 }],
  ["paise", { code: "INR", scale: 0.01 }],
  ["pence", { code: "GBP", scale: 0.01 }],
  ["penny", { code: "GBP", scale: 0.01 }],
  ["pennies", { code: "GBP", scale: 0.01 }],
]);
/** Weights and the like: "1 pound to kg" is not about money. */
const NOT_MONEY = new Set("g gm gms gram grams kg kgs kilo kilos kilogram kilograms mg lb lbs oz ounce ounces ton tons tonne tonnes stone stones".split(" "));
/** The codes worth correcting when a letter is doubled or two are swapped ("usdd", "irn"). */
const COMMON_CODES = ["USD", "INR", "EUR", "GBP", "JPY", "AED", "AUD", "SGD", "NPR", "BDT", "PKR"];
const SYMBOLS = { $: "USD", "€": "EUR", "£": "GBP", "¥": "JPY", "₹": "INR", "₩": "KRW", "₽": "RUB", "₺": "TRY", "฿": "THB", "₫": "VND", "₱": "PHP", "₦": "NGN", "₪": "ILS" };
const PREFIXED_SYMBOLS = { us: "USD", au: "AUD", a: "AUD", ca: "CAD", c: "CAD", hk: "HKD", nz: "NZD", s: "SGD", r: "BRL" };
export const MAGNITUDES = new Map(
  Object.entries({
    k: 1e3, thousand: 1e3, thousands: 1e3, hazar: 1e3, hazaar: 1e3, हजार: 1e3, hundred: 100, hundreds: 100, sau: 100, सौ: 100,
    l: 1e5, lac: 1e5, lacs: 1e5, lakh: 1e5, lakhs: 1e5, लाख: 1e5,
    cr: 1e7, crore: 1e7, crores: 1e7, karod: 1e7, karor: 1e7, करोड़: 1e7,
    m: 1e6, mn: 1e6, million: 1e6, millions: 1e6,
    b: 1e9, bn: 1e9, billion: 1e9, billions: 1e9,
  }),
);
export const NUMBER_WORDS = new Map(
  Object.entries({
    one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8, nine: 9, ten: 10,
    eleven: 11, twelve: 12, thirteen: 13, fourteen: 14, fifteen: 15, sixteen: 16, seventeen: 17, eighteen: 18, nineteen: 19,
    twenty: 20, thirty: 30, forty: 40, fifty: 50, sixty: 60, seventy: 70, eighty: 80, ninety: 90,
    ek: 1, teen: 3, char: 4, chaar: 4, paanch: 5, panch: 5, chhe: 6, saat: 7, aath: 8, nau: 9, das: 10, bees: 20, pachas: 50, एक: 1, दो: 2, तीन: 3, चार: 4, पांच: 5, दस: 10,
  }),
);
/** Words linking the two currencies ("usd to inr", "dollar ka rupee"). */
const DIRECTION_WORDS = new Set(["to", "in", "into", "vs", "versus", "against", "per", "ka", "ki", "ke", "me", "mein", "main", "mai", "se", "में", "का", "की", "के", "से"]);
const POSSESSIVES = new Set(["ka", "ki", "ke", "का", "की", "के"]);
/** Words for money in general: "japan currency", "dubai money". */
const GENERIC = new Set(["currency", "currencies", "money", "cash", "exchange", "forex"]);
/** Words asking for the rate, which make a lone currency a question ("dollar rate"). */
const RATE_WORDS = new Set(["rate", "rates", "price", "value", "worth", "currency", "currencies", "exchange", "forex", "bhav", "bhaav", "daam", "keemat", "kimat", "mulya", "kitna", "kitne", "kitni", "कितना", "कितने", "कितनी", "भाव", "कीमत"]);
const FILLER = new Set(
  `what whats wat wats wht is are was were the of for cost conversion convert converted converter converting conv calculate calc
  change how hw much mch many does do did it its make makes equal equals equivalent current currently today todays now latest
  live please pls plz tell give show find get check at on my from and or than would be can could will should you your need want
  know say exact exactly approx approximately about around roughly like with this that these there has have had if i we am so
  just only hi hello hey bro sir thanks thank right market official amount total
  kya hai hain hoga hota hote hoti hua hue aur wala wale mujhe bataiye batao btao bata aaj abhi ko par barabar yaar bhai
  है हैं क्या आज को बराबर`.split(/\s+/),
);

const UNIT_OF = new Map();
const FORMS_OF = new Map();
const PLACE_OF = new Map();
const PHRASES = [];
for (const [code, units, places] of CURRENCIES) {
  const forms = new Set();
  for (const unit of units) {
    for (const form of [unit, `${unit}s`, ...(SPELLINGS[unit] ?? [])]) {
      forms.add(form);
      if (!UNIT_OF.has(form)) UNIT_OF.set(form, { code, clear: !LOOSE_UNITS.has(unit) });
    }
  }
  FORMS_OF.set(code, forms);
  for (const place of places) {
    const compact = place.replace(/\s+/g, "");
    if (!PLACE_OF.has(compact)) PLACE_OF.set(compact, code);
    if (place.includes(" ")) PHRASES.push(place);
  }
}
const ALL_CODES = CURRENCIES.map(([code]) => code);
/** Names of several words, longest first, are joined into one before reading. */
const JOINED_NAMES = new RegExp(
  `\\b(${PHRASES.sort((a, b) => b.length - a.length).map((p) => p.replace(/ /g, "\\s+")).join("|")})\\b`,
  "gi",
);
/** What a misspelled word may have meant, by first letter; on a tie the more common name wins. */
const TYPO_TARGETS = new Map();
for (const name of [...UNIT_OF.keys(), ...GENERIC, ...PLACE_OF.keys(), ...FILLER, ...RATE_WORDS]) {
  if (name.length < 4 || !/^[a-z]+$/.test(name)) continue;
  if (!TYPO_TARGETS.has(name[0])) TYPO_TARGETS.set(name[0], []);
  TYPO_TARGETS.get(name[0]).push(name);
}

const isClearCode = (code) => !WORD_CODES.has(code);
const isCurrency = (part) => part?.type === "currency";

/** Edits between two words, a swap of neighbours counting as one. */
export function editDistance(a, b) {
  const rows = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) rows[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      rows[i][j] = Math.min(rows[i - 1][j] + 1, rows[i][j - 1] + 1, rows[i - 1][j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        rows[i][j] = Math.min(rows[i][j], rows[i - 2][j - 2] + 1);
      }
    }
  }
  return rows[a.length][b.length];
}

function closest(word) {
  let best = null;
  let bestDistance = 3;
  for (const name of TYPO_TARGETS.get(word[0]) ?? []) {
    const allowed = name.length >= 7 ? 2 : 1;
    if (Math.abs(name.length - word.length) > allowed) continue;
    // a short word is a typo of a filler only if its letters were swapped: "walk" is not "wala"
    if (word.length <= 4 && FILLER.has(name) && [...word].sort().join("") !== [...name].sort().join("")) continue;
    const distance = editDistance(word, name);
    if (distance <= allowed && distance < bestDistance) {
      best = name;
      bestDistance = distance;
    }
  }
  return best;
}

/** "1,000" and "1,00,000" are grouped; "1,5" and "1.000,50" are European decimals. */
export function readNumber(raw) {
  const comma = raw.lastIndexOf(",");
  const dot = raw.lastIndexOf(".");
  if (comma >= 0 && dot >= 0) {
    const [decimal, group] = comma > dot ? [",", "."] : [".", ","];
    return raw.split(group).join("").replace(decimal, ".");
  }
  if (comma >= 0) {
    const groups = raw.split(",");
    return groups.length === 2 && groups[1].length < 3 ? groups.join(".") : groups.join("");
  }
  const groups = raw.split(".");
  return groups.length > 2 ? groups.join("") : raw;
}

/** Puts the query in a form the scanner can read. */
function normalize(text, known) {
  return text
    .replace(/\b(rs)\.(?=\s*\d)/gi, "$1 ")
    .replace(/=>|->|→|=/g, " to ")
    .replace(/\d[\d.,]*\d/g, readNumber)
    .replace(/(^|[^\d.,])(\d{1,3})((?: \d{3})+)(?!\d)/g, (_, before, first, rest) => before + first + rest.replace(/ /g, ""))
    .replace(/['’]/g, "")
    .replace(JOINED_NAMES, (name) => name.replace(/\s+/g, "").toLowerCase())
    .replace(/\b([a-z]{3})2([a-z]{3})\b/gi, (all, a, b) =>
      known.has(a.toUpperCase()) && known.has(b.toUpperCase()) ? `${a} to ${b}` : all,
    )
    .replace(/\.(?!\d)/g, " ")
    .replace(/[?!,:;"“”()[\]{}/\\|_~…–—&>-]/g, " ");
}

/** The meaning of a correctly spelled word, or null. */
function exactToken(word, raw, known) {
  if (MAGNITUDES.has(word)) return { type: "magnitude", value: MAGNITUDES.get(word) };
  if (NUMBER_WORDS.has(word)) return { type: "number", value: NUMBER_WORDS.get(word), spelled: true };
  if (word === "a" || word === "an") return { type: "article" };
  if (word === "us") return { type: "place", code: "USD", isUs: true };
  if (GENERIC.has(word)) return { type: "generic", word };
  if (DIRECTION_WORDS.has(word) || FILLER.has(word) || RATE_WORDS.has(word)) return { type: "word", word };
  if (SUBUNITS.has(word)) return { type: "currency", clear: true, word, ...SUBUNITS.get(word) };
  const unit = UNIT_OF.get(word);
  if (unit) return { type: "currency", code: unit.code, clear: unit.clear, unit: word };
  if (PLACE_OF.has(word)) return { type: "place", code: PLACE_OF.get(word) };
  const code = word.toUpperCase();
  // "usd" or "USD"; a code that is also a word counts in capitals ("100 TRY")
  if (word.length === 3 && known.has(code)) return { type: "currency", code, clear: isClearCode(code) || raw === code };
  return null;
}

/** "usdinr", "usdtoinr", "dollartorupee". */
function splitGlued(word, known) {
  if (word.length < 6) return null;
  const named = (part) => {
    const token = exactToken(part, part, known);
    return token?.type === "currency" || token?.type === "place" ? token : null;
  };
  for (let i = 3; i <= word.length - 3; i++) {
    const left = named(word.slice(0, i));
    if (!left) continue;
    const rest = word.slice(i);
    for (const link of ["to", "in", ""]) {
      if (!rest.startsWith(link)) continue;
      const right = named(rest.slice(link.length));
      if (right) return [left, { type: "word", word: link || "to" }, right];
    }
  }
  return null;
}

function codeTypo(word) {
  if (word.length !== 3 && word.length !== 4) return null;
  return (
    COMMON_CODES.find((code) => {
      const c = code.toLowerCase();
      return word.length === 4 ? word.replace(/(.)\1/, "$1") === c : word[0] === c[0] && word[1] === c[2] && word[2] === c[1];
    }) ?? null
  );
}

function wordToken(raw, known) {
  const word = raw.toLowerCase();
  if (NOT_MONEY.has(word) && raw !== word.toUpperCase()) return { type: "stop" };
  const exact = exactToken(word, raw, known);
  if (exact) return exact;
  const glued = splitGlued(word, known);
  if (glued) return { type: "glued", tokens: glued };
  const typedCode = codeTypo(word);
  if (typedCode) return { type: "currency", code: typedCode, clear: true };
  const near = word.length >= 4 ? closest(word) : null;
  if (near) {
    const token = exactToken(near, near, known);
    if (token?.type === "currency") return { ...token, fuzzy: true, clear: token.clear && CORRECTABLE.has(near) && word.length >= 5 };
    if (token?.type === "place") return { ...token, fuzzy: true };
    if (token) return token;
  }
  return { type: "unknown", word };
}

/**
 * The last word while it is still being typed: after "to" it may already name
 * the currency ("1 usd to nep"); otherwise a possible start of a word is ignored.
 */
function finishWord(word, tokens, known) {
  if (!tokens.some((t) => isCurrency(t) || t.type === "place")) return null;
  if (DIRECTION_WORDS.has(tokens.at(-1)?.word) && word.length >= 2) {
    const name = [...UNIT_OF.keys(), ...PLACE_OF.keys()].find((n) => n.startsWith(word));
    return name ? exactToken(name, name, known) : null;
  }
  const starts =
    [...DIRECTION_WORDS].some((w) => w.startsWith(word)) ||
    [...UNIT_OF.keys(), ...PLACE_OF.keys()].some((n) => n.startsWith(word)) ||
    [...known].some((k) => k.startsWith(word.toUpperCase()));
  return starts ? { type: "dropped" } : null;
}

function scan(text, known) {
  const tokens = [];
  let rest = text;
  while (rest.length) {
    let m;
    if ((m = /^\s+/.exec(rest))) {
      // spaces separate tokens and mean nothing else
    } else if ((m = /^(us|au|ca|hk|nz|a|c|s|r)\$/i.exec(rest))) {
      tokens.push({ type: "currency", code: PREFIXED_SYMBOLS[m[1].toLowerCase()], clear: true, symbol: true });
    } else if ((m = /^[$€£¥₹₩₽₺฿₫₱₦₪]/.exec(rest))) {
      tokens.push({ type: "currency", code: SYMBOLS[m[0]], clear: true, symbol: true });
    } else if ((m = /^(\d+(?:\.\d+)?|\.\d+)(k|mn|bn|m|b|lakhs|lakh|lacs|lac|l|crores|crore|cr)?(?![\p{L}\p{M}\d]|\.\d)/iu.exec(rest))) {
      tokens.push({ type: "number", value: Number(m[1]) * (m[2] ? MAGNITUDES.get(m[2].toLowerCase()) : 1) });
    } else if ((m = /^\d+(?:\.\d+)?|^\.\d+/.exec(rest))) {
      tokens.push({ type: "number", value: Number(m[0]) });
    } else if ((m = /^[\p{L}\p{M}]+/u.exec(rest))) {
      const token = wordToken(m[0], known);
      if (token.type === "glued") {
        tokens.push(...token.tokens);
      } else if (token.type === "unknown" && m[0].length === rest.length) {
        const finished = finishWord(token.word, tokens, known);
        if (finished?.type !== "dropped") tokens.push(finished ?? token);
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

/** "two hundred fifty", "1.5 lakh", "1 crore 50 lakh" become one number. */
function joinNumbers(tokens) {
  const out = [];
  for (let i = 0; i < tokens.length; ) {
    if (tokens[i].type !== "number" && tokens[i].type !== "magnitude") {
      out.push(tokens[i++]);
      continue;
    }
    let total = 0;
    let current = null;
    let joinable = false;
    for (; i < tokens.length; i++) {
      const token = tokens[i];
      if (token.type === "magnitude") {
        if (token.value === 100) current = (current ?? 1) * 100;
        else {
          total += (current ?? 1) * token.value;
          current = null;
        }
        joinable = true;
      } else if (token.type === "number") {
        if (current === null) current = token.value;
        else if (joinable) current += token.value;
        else break;
        joinable = Boolean(token.spelled);
      } else break;
    }
    out.push({ type: "number", value: total + (current ?? 0) });
  }
  return out;
}

/** "nepali rupee", "nepal ka rupee", "japan currency", "currency of nepal" name one currency. */
function joinNames(tokens) {
  const out = [];
  const belongs = (name, place) =>
    name?.type === "generic" || (isCurrency(name) && name.unit && FORMS_OF.get(place.code)?.has(name.unit));
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token.type === "place") {
      const j = POSSESSIVES.has(tokens[i + 1]?.word) ? i + 2 : i + 1;
      // "nepal ka 100 rupee"
      if (tokens[j]?.type === "number" && isCurrency(tokens[j + 1]) && belongs(tokens[j + 1], token)) {
        out.push(tokens[j], { type: "currency", code: token.code, clear: true, named: true });
        i = j + 1;
        continue;
      }
      if (belongs(tokens[j], token)) {
        out.push({ type: "currency", code: token.code, clear: true, named: true });
        i = j;
        continue;
      }
    } else if (tokens[i + 1]?.word === "of" && tokens[i + 2]?.type === "place" && belongs(token, tokens[i + 2])) {
      out.push({ type: "currency", code: tokens[i + 2].code, clear: true, named: true });
      i += 2;
      continue;
    }
    out.push(token);
  }
  return out;
}

/** The query as numbers, currencies and words. */
function assemble(tokens) {
  const parts = joinNames(joinNumbers(tokens))
    .map((part, i, all) => {
      const next = all[i + 1];
      const nextNames = isCurrency(next) || next?.type === "place";
      if (part.type === "article") return nextNames ? { type: "number", value: 1 } : { type: "word", word: "a" };
      // Hindi "do dollar" is two dollars
      if (part.word === "do" && nextNames) return { type: "number", value: 2 };
      if (part.type === "generic") return { type: "word", word: part.word };
      if (part.type === "place") {
        // "us" is a country only after "to" or "in": otherwise "tell us"
        if (part.isUs && !DIRECTION_WORDS.has(all[i - 1]?.word)) return { type: "word", word: "us" };
        return { type: "currency", code: part.code, clear: false, place: true, fuzzy: part.fuzzy };
      }
      return part;
    });
  return parts;
}

/** "$100 usd" is one currency; a word like "to" between them keeps "usd to usd" as two. */
function collapseRepeats(parts) {
  const out = [];
  let last = null;
  let linked = false;
  for (const part of parts) {
    if (isCurrency(part)) {
      if (last?.code === part.code && !linked) {
        last.clear ||= part.clear;
        last.symbol ||= part.symbol;
        last.scale ??= part.scale;
        continue;
      }
      last = part;
      linked = false;
    } else if (DIRECTION_WORDS.has(part.word)) {
      linked = true;
    }
    out.push(part);
  }
  return out;
}

/** "1 usd to inr and eur" answers the first pair; a misspelled extra name is not a third currency. */
function limitToPair(parts) {
  let named = 0;
  const cut = parts.findIndex((part) => {
    if (isCurrency(part)) named++;
    return named >= 2 && (part.word === "and" || part.word === "or");
  });
  const kept = cut >= 0 ? parts.slice(0, cut) : parts;
  if (kept.filter(isCurrency).length <= 2) return kept;
  return kept.map((part) => (isCurrency(part) && part.fuzzy ? { type: "unknown" } : part));
}

/** "1 usd 2 inr": chat shorthand for "to". */
function readTwoAsTo(parts) {
  const numbers = parts.filter((p) => p.type === "number");
  if (numbers.length !== 2) return parts;
  return parts.map((part, i) =>
    part === numbers[1] && part.value === 2 && isCurrency(parts[i - 1]) && isCurrency(parts[i + 1])
      ? { type: "word", word: "to" }
      : part,
  );
}

/** Which of the two currencies the amount is in. */
function sourceOf(parts, first, second) {
  const n = parts.findIndex((p) => p.type === "number");
  if (n < 0) {
    // "inr value of usd", "rupees per dollar"
    const between = parts.slice(parts.indexOf(first) + 1, parts.indexOf(second));
    return between.some((p) => p.word === "of" || p.word === "per") ? second : first;
  }
  const previous = parts[n - 1];
  const next = parts[n + 1];
  if (previous?.symbol) return previous;
  if (next?.symbol) return next;
  if (n > parts.indexOf(second)) return first;
  if (previous?.type === "currency") return previous;
  if (next?.type === "currency") return next;
  return first;
}

/** Whether these two names are clearly a conversion, not an ordinary search ("india vs pakistan", "i won 100 dollars"). */
function isClearPair(parts, first, second) {
  const between = parts.slice(parts.indexOf(first) + 1, parts.indexOf(second));
  if (!first.clear && !second.clear) {
    const hasNumber = parts.some((p) => p.type === "number");
    return hasNumber && first.place && second.place && between.some((p) => ["to", "in", "into"].includes(p.word));
  }
  if ((first.clear || first.place) && (second.clear || second.place)) return true;
  return between.some((p) => DIRECTION_WORDS.has(p.word));
}

/**
 * `{ amount, from, to }` if `text` asks for a currency conversion, else null.
 * Understands all the service's currencies, their countries, typos and how
 * people phrase it. A word it cannot place inside the conversion means no
 * answer, because a wrong currency is worse than none.
 */
export function parseConversion(text, codes = []) {
  const trimmed = text.trim();
  if (!trimmed || trimmed.startsWith("-")) return null;
  const known = new Set([...ALL_CODES, ...codes]);
  const tokens = scan(normalize(trimmed, known), known);
  if (!tokens || tokens.some((t) => t.type === "stop")) return null;
  const parts = readTwoAsTo(collapseRepeats(limitToPair(assemble(tokens))));

  const currencies = parts.filter(isCurrency);
  const numbers = parts.filter((p) => p.type === "number");
  if (currencies.length < 1 || currencies.length > 2 || numbers.length > 1) return null;

  if (parts.some((p) => p.type === "unknown")) {
    // words around a clear "100 usd to inr" are fine; words inside it change what it means
    const anchors = parts.flatMap((p, i) => (isCurrency(p) || p.type === "number" ? [i] : []));
    const inside = parts.slice(anchors[0], anchors.at(-1) + 1);
    if (currencies.length < 2 || inside.some((p) => p.type === "unknown")) return null;
  }

  const [first, second] = currencies;
  if (second ? !isClearPair(parts, first, second) : !first.clear) return null;
  // a lone "dollar" is still a search; "dollar rate" and "nepali rupee" ask for the currency
  if (!numbers.length && !second && !first.named && !parts.some((p) => RATE_WORDS.has(p.word))) return null;

  // "euro 2024" is the football
  const year = numbers[0]?.value;
  if (!second && /^euros?$/.test(first.unit ?? "") && Number.isInteger(year) && year >= 1960 && year <= 2100) {
    if (parts.indexOf(first) < parts.indexOf(numbers[0])) return null;
  }

  const source = second ? sourceOf(parts, first, second) : first;
  const amount = (numbers[0]?.value ?? 1) * (source.scale ?? 1);
  if (!Number.isFinite(amount) || amount > MAX_AMOUNT) return null;
  const other = first.code === HOME ? AWAY : HOME;
  if (!second) {
    // "500 to usd" names only the target, so the amount is in the home currency
    const i = parts.indexOf(first);
    const isTarget = ["to", "in", "into"].includes(parts[i - 1]?.word) && parts.findIndex((p) => p.type === "number") < i - 1;
    return isTarget && numbers.length ? { amount, from: other, to: first.code } : { amount, from: first.code, to: other };
  }
  return { amount, from: source.code, to: (source === first ? second : first).code };
}

/** "₹1,00,000.50", in the digit grouping of that currency's home for rupees. */
export function formatMoney(value, code, { whole = false } = {}) {
  const digits =
    value !== 0 && Math.abs(value) < 1
      ? { maximumSignificantDigits: 4 }
      : whole && Number.isInteger(value)
        ? { minimumFractionDigits: 0, maximumFractionDigits: 0 }
        : { minimumFractionDigits: 2, maximumFractionDigits: 2 };
  try {
    return new Intl.NumberFormat(code === "INR" ? "en-IN" : "en-US", { style: "currency", currency: code, ...digits }).format(value);
  } catch {
    return `${value} ${code}`;
  }
}

function updatedOn(seconds) {
  return new Date(seconds * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", timeZone: "UTC" });
}

/** "US Dollar" for "USD". */
function currencyName(code) {
  try {
    return new Intl.DisplayNames(["en"], { type: "currency" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/**
 * What Spotlight shows for `text`, or null if it is not a conversion:
 * `{ amount, from, to, fromName, toName, source, answer, rate, updated }`, the
 * last three null until rates are saved.
 */
export function answerConversion(text, saved) {
  const parsed = parseConversion(text, Object.keys(saved?.rates ?? {}));
  if (!parsed) return null;
  const { amount, from, to } = parsed;
  const sides = {
    amount,
    from,
    to,
    fromName: currencyName(from),
    toName: currencyName(to),
    source: formatMoney(amount, from, { whole: true }),
  };
  const fromRate = saved?.rates?.[from];
  const toRate = saved?.rates?.[to];
  if (!(fromRate > 0 && toRate > 0)) return { ...sides, answer: null, rate: null, updated: null };
  const rate = toRate / fromRate;
  return {
    ...sides,
    answer: formatMoney(amount * rate, to),
    rate: `1 ${from} = ${formatMoney(rate, to)}`,
    updated: `Updated ${updatedOn(saved.updatedAt)}`,
  };
}

/** Local results shown below a conversion at most, so that the list still fits without scrolling. */
const MAX_RELATED = 3;
/** Names too common or too short to tie a file to a currency. */
const NOT_RELATED = new Set(["buck", "bucks", "quid", "rupe"]);
/** Files someone would want next to a conversion: invoices and sheets, not a project's `currency.js`. */
const USEFUL_FILE = /\.(pdf|docx?|odt|rtf|txt|xlsx?|ods|csv|tsv|pptx?|odp|png|jpe?g|webp)$/i;
const isUseful = (result) =>
  result.kind === "app" || result.kind === "setting" || (result.kind === "file" && USEFUL_FILE.test(result.title));

/** The currency's everyday name ("dollar" for CAD), unless that is also an ordinary word ("won"). */
function everydayName(code) {
  const name = currencyName(code).toLowerCase().split(/\s+/).at(-1);
  return UNIT_OF.get(name)?.clear ? name : null;
}

/** What to look for on this computer for a conversion between two currencies. */
export function relatedTerms(from, to) {
  const terms = [from, to].flatMap((code) => [isClearCode(code) ? code.toLowerCase() : null, everydayName(code)]);
  return [...new Set([...terms.filter(Boolean), "currency"])];
}

function relevanceWords(from, to) {
  const words = new Set(["currency", "currencies", "forex"]);
  for (const code of [from, to]) {
    if (isClearCode(code)) words.add(code.toLowerCase());
    for (const form of FORMS_OF.get(code) ?? []) {
      if (form.length >= 4 && !NOT_RELATED.has(form) && UNIT_OF.get(form)?.clear) words.add(form);
    }
    const name = everydayName(code);
    if (name) words.add(name).add(`${name}s`);
  }
  return words;
}

/**
 * The apps, settings and documents among `responses` whose name has one of
 * these currencies as a whole word ("Dollar invoice.pdf", not "Dollhouse.png"),
 * best first.
 */
export function relevantResults(responses, from, to) {
  const words = relevanceWords(from, to);
  const found = new Map();
  for (const response of responses) {
    for (const section of response?.sections ?? []) {
      for (const result of section.results) {
        const named = result.title.toLowerCase().split(/[^\p{L}\p{N}]+/u).some((word) => words.has(word));
        if (isUseful(result) && named && !found.has(result.id)) found.set(result.id, result);
      }
    }
  }
  return [...found.values()].sort((a, b) => b.score - a.score).slice(0, MAX_RELATED);
}
