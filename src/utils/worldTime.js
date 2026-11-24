// Spotlight's world clock answers ("time in Dubai", "what time is it in new york").
// No network: this computer already knows the time and its own time zone, and
// the web engine carries the time zone database (daylight saving included), so
// the other city's time is the same instant read in that city's zone.

/** IANA time zone, then the names it is asked for by: cities, countries, short zone names. */
const PLACES = [
  ["Asia/Dubai", "dubai", "abu dhabi", "sharjah", "uae", "united arab emirates", "gst", "gulf"],
  ["Asia/Kolkata", "india", "delhi", "new delhi", "mumbai", "bombay", "kolkata", "calcutta", "chennai", "madras", "bangalore", "bengaluru", "hyderabad", "pune", "ahmedabad", "jaipur", "lucknow", "siliguri", "guwahati", "ist", "indian standard time"],
  ["Asia/Karachi", "pakistan", "karachi", "lahore", "islamabad", "pkt"],
  ["Asia/Dhaka", "bangladesh", "dhaka"],
  ["Asia/Kathmandu", "nepal", "kathmandu"],
  ["Asia/Colombo", "sri lanka", "colombo"],
  ["Asia/Thimphu", "bhutan", "thimphu"],
  ["Indian/Maldives", "maldives", "male"],
  ["Asia/Kabul", "afghanistan", "kabul"],
  ["Asia/Tashkent", "uzbekistan", "tashkent"],
  ["Asia/Almaty", "kazakhstan", "almaty", "astana"],
  ["Asia/Tehran", "iran", "tehran"],
  ["Asia/Baghdad", "iraq", "baghdad"],
  ["Asia/Riyadh", "saudi arabia", "saudi", "riyadh", "jeddah", "mecca", "makkah", "medina"],
  ["Asia/Kuwait", "kuwait"],
  ["Asia/Qatar", "qatar", "doha"],
  ["Asia/Bahrain", "bahrain", "manama"],
  ["Asia/Muscat", "oman", "muscat"],
  ["Asia/Jerusalem", "israel", "jerusalem", "tel aviv"],
  ["Asia/Beirut", "lebanon", "beirut"],
  ["Asia/Amman", "jordan", "amman"],
  ["Asia/Damascus", "syria", "damascus"],
  ["Europe/Istanbul", "turkey", "turkiye", "istanbul", "ankara"],
  ["Asia/Tbilisi", "georgia country", "tbilisi"],
  ["Asia/Yerevan", "armenia", "yerevan"],
  ["Asia/Baku", "azerbaijan", "baku"],
  ["Asia/Bangkok", "thailand", "bangkok", "phuket"],
  ["Asia/Ho_Chi_Minh", "vietnam", "hanoi", "ho chi minh", "saigon"],
  ["Asia/Singapore", "singapore", "sgt"],
  ["Asia/Kuala_Lumpur", "malaysia", "kuala lumpur"],
  ["Asia/Jakarta", "indonesia", "jakarta"],
  ["Asia/Makassar", "bali", "denpasar"],
  ["Asia/Manila", "philippines", "manila"],
  ["Asia/Hong_Kong", "hong kong", "hongkong", "hkt"],
  ["Asia/Macau", "macau", "macao"],
  ["Asia/Shanghai", "china", "beijing", "shanghai", "shenzhen", "guangzhou", "chengdu"],
  ["Asia/Taipei", "taiwan", "taipei"],
  ["Asia/Seoul", "south korea", "korea", "seoul", "kst"],
  ["Asia/Pyongyang", "north korea", "pyongyang"],
  ["Asia/Tokyo", "japan", "tokyo", "osaka", "kyoto", "jst"],
  ["Asia/Yangon", "myanmar", "burma", "yangon", "rangoon"],
  ["Asia/Phnom_Penh", "cambodia", "phnom penh"],
  ["Asia/Vientiane", "laos", "vientiane"],
  ["Asia/Brunei", "brunei"],
  ["Asia/Ulaanbaatar", "mongolia", "ulaanbaatar", "ulan bator"],
  ["Asia/Vladivostok", "vladivostok"],
  ["Europe/Moscow", "russia", "moscow", "st petersburg", "saint petersburg", "msk"],
  ["Asia/Yekaterinburg", "yekaterinburg"],
  ["Asia/Novosibirsk", "novosibirsk"],
  ["Europe/London", "uk", "united kingdom", "england", "britain", "great britain", "london", "scotland", "edinburgh", "manchester", "wales", "bst"],
  ["Europe/Dublin", "ireland", "dublin"],
  ["Europe/Lisbon", "portugal", "lisbon", "porto"],
  ["Atlantic/Reykjavik", "iceland", "reykjavik"],
  ["Europe/Paris", "france", "paris", "marseille", "lyon", "cet", "cest", "central european time"],
  ["Europe/Berlin", "germany", "berlin", "munich", "frankfurt", "hamburg"],
  ["Europe/Madrid", "spain", "madrid", "barcelona"],
  ["Europe/Rome", "italy", "rome", "milan", "venice", "naples"],
  ["Europe/Amsterdam", "netherlands", "holland", "amsterdam"],
  ["Europe/Brussels", "belgium", "brussels"],
  ["Europe/Zurich", "switzerland", "zurich", "geneva", "bern"],
  ["Europe/Vienna", "austria", "vienna"],
  ["Europe/Prague", "czech republic", "czechia", "prague"],
  ["Europe/Warsaw", "poland", "warsaw", "krakow"],
  ["Europe/Budapest", "hungary", "budapest"],
  ["Europe/Stockholm", "sweden", "stockholm"],
  ["Europe/Oslo", "norway", "oslo"],
  ["Europe/Copenhagen", "denmark", "copenhagen"],
  ["Europe/Helsinki", "finland", "helsinki"],
  ["Europe/Athens", "greece", "athens", "eet"],
  ["Europe/Bucharest", "romania", "bucharest"],
  ["Europe/Sofia", "bulgaria", "sofia"],
  ["Europe/Belgrade", "serbia", "belgrade"],
  ["Europe/Zagreb", "croatia", "zagreb"],
  ["Europe/Kyiv", "ukraine", "kyiv", "kiev"],
  ["Europe/Minsk", "belarus", "minsk"],
  ["Europe/Riga", "latvia", "riga"],
  ["Europe/Vilnius", "lithuania", "vilnius"],
  ["Europe/Tallinn", "estonia", "tallinn"],
  ["Europe/Luxembourg", "luxembourg"],
  ["Europe/Malta", "malta", "valletta"],
  ["Asia/Nicosia", "cyprus", "nicosia"],
  ["Africa/Cairo", "egypt", "cairo"],
  ["Africa/Johannesburg", "south africa", "johannesburg", "cape town", "pretoria", "durban", "sast"],
  ["Africa/Lagos", "nigeria", "lagos", "abuja", "wat"],
  ["Africa/Nairobi", "kenya", "nairobi", "east africa", "eat"],
  ["Africa/Addis_Ababa", "ethiopia", "addis ababa"],
  ["Africa/Dar_es_Salaam", "tanzania", "dar es salaam"],
  ["Africa/Kampala", "uganda", "kampala"],
  ["Africa/Accra", "ghana", "accra"],
  ["Africa/Casablanca", "morocco", "casablanca", "rabat"],
  ["Africa/Algiers", "algeria", "algiers"],
  ["Africa/Tunis", "tunisia", "tunis"],
  ["Africa/Tripoli", "libya", "tripoli"],
  ["Africa/Khartoum", "sudan", "khartoum"],
  ["Africa/Kinshasa", "kinshasa"],
  ["Africa/Harare", "zimbabwe", "harare"],
  ["Africa/Lusaka", "zambia", "lusaka"],
  ["Africa/Maputo", "mozambique", "maputo"],
  ["Africa/Dakar", "senegal", "dakar"],
  ["Indian/Mauritius", "mauritius"],
  ["America/New_York", "new york", "nyc", "new york city", "usa", "us", "united states", "america", "washington", "washington dc", "boston", "miami", "atlanta", "philadelphia", "orlando", "detroit", "florida", "georgia us", "est", "edt", "eastern", "eastern time", "et"],
  ["America/Chicago", "chicago", "dallas", "houston", "texas", "austin", "new orleans", "minneapolis", "cst", "cdt", "central time"],
  ["America/Denver", "denver", "colorado", "salt lake city", "utah", "mst", "mdt", "mountain time"],
  ["America/Phoenix", "phoenix", "arizona"],
  ["America/Los_Angeles", "los angeles", "la", "san francisco", "sf", "seattle", "california", "san diego", "las vegas", "portland", "silicon valley", "pst", "pdt", "pacific", "pacific time", "pt"],
  ["America/Anchorage", "alaska", "anchorage"],
  ["Pacific/Honolulu", "hawaii", "honolulu"],
  ["America/Toronto", "canada", "toronto", "ottawa", "montreal", "quebec"],
  ["America/Vancouver", "vancouver", "british columbia"],
  ["America/Edmonton", "calgary", "edmonton", "alberta"],
  ["America/Winnipeg", "winnipeg"],
  ["America/Halifax", "halifax", "nova scotia"],
  ["America/St_Johns", "newfoundland", "st johns"],
  ["America/Mexico_City", "mexico", "mexico city"],
  ["America/Guatemala", "guatemala"],
  ["America/Costa_Rica", "costa rica"],
  ["America/Panama", "panama"],
  ["America/Havana", "cuba", "havana"],
  ["America/Jamaica", "jamaica", "kingston"],
  ["America/Puerto_Rico", "puerto rico"],
  ["America/Bogota", "colombia", "bogota"],
  ["America/Lima", "peru", "lima"],
  ["America/Caracas", "venezuela", "caracas"],
  ["America/Santiago", "chile", "santiago"],
  ["America/Argentina/Buenos_Aires", "argentina", "buenos aires"],
  ["America/Sao_Paulo", "brazil", "sao paulo", "rio", "rio de janeiro", "brasilia"],
  ["America/La_Paz", "bolivia", "la paz"],
  ["America/Montevideo", "uruguay", "montevideo"],
  ["America/Asuncion", "paraguay", "asuncion"],
  ["America/Guayaquil", "ecuador", "quito"],
  ["Australia/Sydney", "australia", "sydney", "melbourne", "canberra", "tasmania", "hobart", "aest", "aedt"],
  ["Australia/Brisbane", "brisbane", "queensland"],
  ["Australia/Adelaide", "adelaide"],
  ["Australia/Darwin", "darwin"],
  ["Australia/Perth", "perth", "awst"],
  ["Pacific/Auckland", "new zealand", "auckland", "wellington", "nz", "nzst"],
  ["Pacific/Fiji", "fiji", "suva"],
  ["Pacific/Guam", "guam"],
  ["Pacific/Port_Moresby", "papua new guinea", "port moresby"],
  ["Africa/Abidjan", "ivory coast", "cote divoire", "cote d ivoire", "abidjan"],
  ["Africa/Mbabane", "eswatini", "swaziland"],
  ["Atlantic/Cape_Verde", "cape verde", "cabo verde"],
  ["Asia/Dili", "east timor", "timor leste"],
  ["Europe/Skopje", "north macedonia", "macedonia"],
  ["Asia/Gaza", "palestine", "gaza"],
  ["Europe/Vatican", "vatican city", "vatican"],
  ["UTC", "utc", "gmt", "zulu", "z", "greenwich", "coordinated universal time", "universal time"],
];

import { inCaseOf } from "./suggestCase";

/** The places (about 800 KB), loaded when Spotlight first opens rather than with the page. */
let data = null;
let loading = null;
export function loadPlaces() {
  loading ??= import("../data/places.json").then(
    (module) => {
      data = module.default;
      return data;
    },
    (error) => {
      // asked for again next time, rather than failing for good
      loading = null;
      throw error;
    },
  );
  return loading;
}

/** São Paulo, "sao  paulo!" and "saopaulo" are the same place. */
const fold = (text) =>
  text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9 ]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
const compact = (text) => fold(text).replace(/ /g, "");

/** Time zone ids this web engine may know under their older name, and the other way round. */
const RENAMED = [
  ["Europe/Kyiv", "Europe/Kiev"],
  ["Asia/Kolkata", "Asia/Calcutta"],
  ["Asia/Yangon", "Asia/Rangoon"],
  ["Asia/Ho_Chi_Minh", "Asia/Saigon"],
  ["Asia/Kathmandu", "Asia/Katmandu"],
  ["America/Nuuk", "America/Godthab"],
  ["Pacific/Kanton", "Pacific/Enderbury"],
  ["Atlantic/Faroe", "Atlantic/Faeroe"],
  ["Africa/Asmara", "Africa/Asmera"],
  ["America/Ciudad_Juarez", "America/Ojinaga"],
];

const known = new Map();
/** The zone as this engine spells it, or null when it has no such zone. */
function usableZone(zone) {
  if (known.has(zone)) return known.get(zone);
  const names = [zone, ...RENAMED.filter((pair) => pair.includes(zone)).flat()];
  const found =
    names.find((name) => {
      try {
        new Intl.DateTimeFormat("en", { timeZone: name });
        return true;
      } catch {
        return false;
      }
    }) ?? null;
  known.set(zone, found);
  return found;
}

/** Short names are shown in capitals ("IST"), every other name as a name. */
const ABBREVIATIONS = new Set("gst ist pkt sgt hkt kst jst msk cet cest eet bst est edt cst cdt mst mdt pst pdt pt et utc gmt z uk us usa uae la sf nyc nz wat eat sast aest aedt awst nzst".split(" "));
const title = (text) => text.replace(/\b[a-z]/g, (c) => c.toUpperCase());

/** What people call a country when it is not its official name. */
const COUNTRY_ALIASES = { usa: "unitedstates", us: "unitedstates", america: "unitedstates", uk: "unitedkingdom", britain: "unitedkingdom", uae: "unitedarabemirates" };

/** Words that are never meant as a place, so a near miss ("table") is not taken for one. */
const NOT_PLACES = new Set(
  "table tables machine travel tracker tracking series management sheet card cards lapse limit line lines stamp saver keeper keeping zone zones settings setting clock watch format change display sync server share sharing capsule bomb attack output input check checker budget plan planner stories story card".split(" "),
);

let index = null;
function places() {
  if (index) return index;
  if (!data) return null;
  const byKey = new Map();
  const keys = [];
  const put = (display, zone, key = compact(display)) => {
    if (key && !byKey.has(key)) {
      byKey.set(key, { display, zone, order: keys.length });
      keys.push(key);
    }
  };
  for (const [zone, ...names] of PLACES) {
    for (const name of names) put(ABBREVIATIONS.has(name) ? name.toUpperCase() : title(name), zone);
  }
  for (const [display, zone] of data.places) put(display, data.zones[zone]);
  for (const [alias, place] of data.alts) put(alias, data.zones[data.places[place][1]]);
  // the same name in several countries: the biggest has the plain name, the others go by "name, country" or "name, state"
  const repeated = new Map();
  for (const [display, zone, country, state] of data.dups) {
    for (const qualifier of [country, state]) {
      if (qualifier) {
        const key = `${compact(display)}|${compact(qualifier)}`;
        if (!repeated.has(key)) repeated.set(key, { display, zone: data.zones[zone] });
      }
    }
  }
  const buckets = new Map();
  for (const key of keys) {
    if (!buckets.has(key.length)) buckets.set(key.length, []);
    buckets.get(key.length).push(key);
  }
  index = { byKey, keys, buckets, repeated };
  return index;
}

/** Edits (adding, dropping, changing a letter or swapping two) between two words, or more than `max`. */
export function editDistance(a, b, max) {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  let previous = Array.from({ length: b.length + 1 }, (_, j) => j);
  let before = previous.slice();
  for (let i = 1; i <= a.length; i++) {
    const row = [i];
    let best = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let d = Math.min(previous[j] + 1, row[j - 1] + 1, previous[j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d = Math.min(d, before[j - 2] + 1);
      row[j] = d;
      best = Math.min(best, d);
    }
    if (best > max) return max + 1;
    before = previous;
    previous = row;
  }
  return previous[b.length];
}

const mistakesAllowed = (length) => (length >= 9 ? 2 : length >= 5 ? 1 : 0);

/** The place whose name `key` misspells, if there is one close enough: the closest, then the biggest. */
function nearest(key) {
  const { byKey, buckets } = index;
  const max = mistakesAllowed(key.length);
  if (!max) return null;
  let best = null;
  for (let length = key.length - max; length <= key.length + max; length++) {
    for (const candidate of buckets.get(length) ?? []) {
      // a first letter is rarely the one that is wrong
      if (candidate[0] !== key[0]) continue;
      const distance = editDistance(key, candidate, max);
      if (distance <= max && (!best || distance < best.distance || (distance === best.distance && byKey.get(candidate).order < best.order))) {
        best = { distance, candidate, order: byKey.get(candidate).order };
      }
    }
  }
  return best ? byKey.get(best.candidate) : null;
}

/** The place a name is the start of ("afgh"), the biggest first. */
function startingWith(key) {
  const { byKey, keys } = index;
  const match = keys.find((candidate) => candidate.startsWith(key));
  return match ? byKey.get(match) : null;
}

/** The place a person typed: exactly, as "name, country", then with typing mistakes, then unfinished. */
export function findPlace(text) {
  const words = fold(text).split(" ").filter(Boolean);
  if (!words.length || !places()) return null;
  const { byKey, repeated } = index;
  const key = words.join("");
  const exact = byKey.get(key) ?? byKey.get(`the${key}`);
  // "paris texas", "london canada": a place named like a bigger one
  for (let split = words.length - 1; split >= 1; split--) {
    const city = words.slice(0, split).join("");
    const where = words.slice(split).join("");
    const qualified = repeated.get(`${city}|${COUNTRY_ALIASES[where] ?? where}`);
    if (qualified) return { zone: qualified.zone, label: qualified.display };
    if (!exact && byKey.has(city) && (byKey.has(where) || COUNTRY_ALIASES[where])) {
      const found = byKey.get(city);
      return { zone: found.zone, label: found.display };
    }
  }
  const found = exact ?? (NOT_PLACES.has(key) ? null : (nearest(key) ?? (key.length >= 4 ? startingWith(key) : null)));
  return found ? { zone: found.zone, label: found.display } : null;
}

// Words around the place that are not the place: "what is the time in Dubai right now",
// "dubai ka time kya hai", "dubai me abhi kitne baje hai".
const TIME_WORDS = new Set("time tiem tmie timr tme timee tyme tiime timw samay waqt baje baja bajey".split(" "));
const AROUND = new Set(
  "what whats wat wats wht s is a an the current currently curent local live now right today please tell me show check get give it kya hai ho ka ki ke mein main mai abhi kitna kitne at in of for from here there rn".split(" "),
);

/** The place asked about: `undefined` when this is not a question about the time somewhere. */
export function parseTimeQuery(text) {
  const words = fold(text).split(" ").filter(Boolean);
  if (!words.some((word) => TIME_WORDS.has(word))) return undefined;
  const rest = words.filter((word) => !TIME_WORDS.has(word));
  let from = 0;
  let to = rest.length;
  while (from < to && AROUND.has(rest[from])) from++;
  while (to > from && AROUND.has(rest[to - 1])) to--;
  const asked = rest.slice(from, to);
  // "time" or "time in" has no place yet: nothing to answer
  if (!asked.length) return undefined;
  // "time zone dubai": the word zone only describes what is wanted
  const found = findPlace(asked.join(" ")) ?? findPlace(asked.filter((word) => word !== "zone" && word !== "timezone").join(" "));
  return found ?? undefined;
}

/** What "time" is completed to when nothing more is typed. */
const SUGGESTED_PLACE = "New York";
const ASKING = "time in ";
/** Tried first when a place is being finished, so "lo" is London and "n" stays New York. */
const POPULAR = ["New York", "London", "Los Angeles", "Dubai", "Tokyo", "Paris", "Singapore", "Sydney", "Hong Kong", "Mumbai", "Delhi", "Chicago", "Toronto", "San Francisco", "Berlin", "Moscow", "Beijing", "Shanghai", "Kolkata", "Istanbul", "Cairo", "Karachi", "Lahore", "Dhaka", "Bangkok", "Seoul", "Madrid", "Rome", "Amsterdam", "Zurich", "Mexico City", "Sao Paulo", "Johannesburg", "Nairobi", "Lagos", "Kathmandu"];

/**
 * The search as it would most likely go on, to show as a suggestion after the
 * typed text: "time" becomes "time in New York", and an unfinished place is
 * finished ("time in lon" → "time in London"). It stays for as long as what is
 * typed fits, in whatever case it is typed in. `null` when there is none.
 */
export function suggestTime(text) {
  const lower = text.toLowerCase();
  if (lower.length >= 4 && ASKING.startsWith(lower)) return text + ASKING.slice(lower.length) + SUGGESTED_PLACE;
  if (!lower.startsWith(ASKING) || !places()) return null;
  const typed = text.slice(ASKING.length);
  const wanted = typed.toLowerCase();
  if (!wanted.trim() || typed.length > 40) return null;
  const finished = (name) => (name.length > wanted.length ? text + inCaseOf(typed, name.slice(wanted.length)) : null);
  const popular = POPULAR.find((name) => name.toLowerCase().startsWith(wanted));
  if (popular) return finished(popular);
  for (const place of index.byKey.values()) {
    // the biggest place of that start; none to add when that is the one typed (London, not Londonderry)
    if (place.display.toLowerCase().startsWith(wanted)) return finished(place.display);
  }
  return null;
}

/** This computer's own time zone, from the system. */
export function localZone() {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

/** Minutes the zone is ahead of UTC at this moment (daylight saving included). */
export function offsetMinutes(zone, date) {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      hourCycle: "h23",
      year: "numeric",
      month: "numeric",
      day: "numeric",
      hour: "numeric",
      minute: "numeric",
      second: "numeric",
    })
      .formatToParts(date)
      .map((p) => [p.type, Number(p.value)]),
  );
  const asUtc = Date.UTC(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute, parts.second);
  return Math.round((asUtc - Math.floor(date.getTime() / 1000) * 1000) / 60000);
}

const clock = (zone, date) => new Intl.DateTimeFormat(undefined, { timeZone: zone, hour: "numeric", minute: "2-digit" }).format(date);
const dayKey = (zone, date) => new Intl.DateTimeFormat("en-CA", { timeZone: zone }).format(date);
const zoneName = (zone, date) =>
  new Intl.DateTimeFormat("en-US", { timeZone: zone, timeZoneName: "long" })
    .formatToParts(date)
    .find((p) => p.type === "timeZoneName")?.value ?? zone;

function dayWord(zone, there, date) {
  const days = Math.round((Date.parse(dayKey(there, date)) - Date.parse(dayKey(zone, date))) / 86400000);
  return days === 0 ? "Today" : days === 1 ? "Tomorrow" : days === -1 ? "Yesterday" : days > 0 ? `In ${days} days` : `${-days} days ago`;
}

/** "3 h 30 min ahead of you", "same time as you". */
export function differenceText(minutes) {
  if (minutes === 0) return "Same time as you";
  const total = Math.abs(minutes);
  const hours = Math.floor(total / 60);
  const rest = total % 60;
  const amount = [hours ? `${hours} h` : "", rest ? `${rest} min` : ""].filter(Boolean).join(" ");
  return `${amount} ${minutes > 0 ? "ahead of" : "behind"} you`;
}

/**
 * The answer to "time in Dubai", shaped like a conversion for `ConversionRow`:
 * your time on the left, theirs on the right. `now` and `zone` are for tests.
 */
export function answerTime(text, { now = new Date(), zone = localZone() } = {}) {
  const place = parseTimeQuery(text);
  if (!place) return null;
  // a zone this web engine lacks is no answer rather than a wrong one
  const there = usableZone(place.zone);
  if (!there) return null;
  place.zone = there;
  const difference = offsetMinutes(place.zone, now) - offsetMinutes(zone, now);
  const answer = clock(place.zone, now);
  return {
    measure: true,
    time: true,
    section: "Time",
    amount: 1,
    from: "Your time",
    to: place.label,
    toName: zoneName(place.zone, now),
    source: clock(zone, now),
    answer,
    rate: `${differenceText(difference)} · ${dayWord(zone, place.zone, now)}`,
    updated: new Intl.DateTimeFormat(undefined, { timeZone: zone, weekday: "short", day: "numeric", month: "short" }).format(now),
    webQuery: `time in ${place.label}`,
  };
}
