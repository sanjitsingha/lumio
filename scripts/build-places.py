#!/usr/bin/env python3
"""Builds src/data/places.json, the places Spotlight's world clock knows: every
country, state and city of 15,000 people or more with its time zone.

    python3 scripts/build-places.py <folder with cities15000.txt countryInfo.txt admin1CodesASCII.txt>

The files are GeoNames data (https://download.geonames.org/export/dump/,
CC BY 4.0, credited in the README). Run it again only to refresh the data.
"""
import collections, json, re, sys, unicodedata
from pathlib import Path

src = Path(sys.argv[1])
out = Path(__file__).resolve().parent.parent / "src" / "data" / "places.json"

def fold(text):
    text = unicodedata.normalize("NFD", text)
    text = "".join(c for c in text if not unicodedata.combining(c)).lower()
    return re.sub(r"[^a-z0-9]+", "", text)

countries = {}
for line in (src / "countryInfo.txt").read_text(encoding="utf-8").splitlines():
    if line.startswith("#") or not line.strip():
        continue
    f = line.split("\t")
    countries[f[0]] = {"name": f[4], "capital": f[5]}

admin1 = {}
for line in (src / "admin1CodesASCII.txt").read_text(encoding="utf-8").splitlines():
    code, name, ascii_name, _ = line.split("\t")
    admin1[code] = ascii_name or name

cities = []
for line in (src / "cities15000.txt").read_text(encoding="utf-8").splitlines():
    f = line.split("\t")
    name, ascii_name = f[1], f[2]
    display = name if fold(name) == fold(ascii_name) else ascii_name
    cities.append({
        "display": display, "alts": f[3].split(",") if f[3] else [], "cc": f[8], "admin1": f[10],
        "pop": int(f[14] or 0), "zone": f[17],
    })
cities.sort(key=lambda c: -c["pop"])

zones, zone_index = [], {}
def z(name):
    if name not in zone_index:
        zone_index[name] = len(zones)
        zones.append(name)
    return zone_index[name]

places, taken, alt_list, dups = [], {}, [], []

def add(display, zone):
    key = fold(display)
    if not key or key in taken:
        return None
    taken[key] = len(places)
    places.append([display, z(zone)])
    return taken[key]

# countries first, at their capital's time zone
by_country = collections.defaultdict(list)
for c in cities:
    by_country[c["cc"]].append(c)
for cc, info in sorted(countries.items(), key=lambda kv: -sum(c["pop"] for c in by_country.get(kv[0], []))):
    local = by_country.get(cc)
    if not local:
        continue
    capital = next((c for c in local if fold(c["display"]) == fold(info["capital"])), local[0])
    add(info["name"], capital["zone"])

# then cities, biggest first; a repeated name keeps the bigger city and lists the others by place
for c in cities:
    country = countries.get(c["cc"], {}).get("name", "")
    state = admin1.get(f'{c["cc"]}.{c["admin1"]}', "")
    index = add(c["display"], c["zone"])
    c["index"] = index
    if index is None and (country or state):
        dups.append([c["display"], z(c["zone"]), country, state])

# states and regions, at the time zone most of their cities share
zones_of = collections.defaultdict(collections.Counter)
for c in cities:
    zones_of[f'{c["cc"]}.{c["admin1"]}'][c["zone"]] += c["pop"]
for code, counter in zones_of.items():
    if code in admin1:
        add(admin1[code], counter.most_common(1)[0][0])

# other names big cities go by (Bombay, Bangalore, Peking…)
plain = re.compile(r"^[A-Za-z][A-Za-z .'-]{2,}$")
for c in cities:
    if c["pop"] < 300000 or c["index"] is None:
        continue
    kept = 0
    for alias in c["alts"]:
        if kept == 5:
            break
        if plain.match(alias) and fold(alias) not in taken:
            taken[fold(alias)] = c["index"]
            alt_list.append([alias, c["index"]])
            kept += 1

data = {"zones": zones, "places": places, "alts": alt_list, "dups": dups}
out.parent.mkdir(exist_ok=True)
out.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
print(f"{len(places)} places, {len(alt_list)} other names, {len(dups)} repeated names, {len(zones)} zones, {out.stat().st_size // 1024} KB")
