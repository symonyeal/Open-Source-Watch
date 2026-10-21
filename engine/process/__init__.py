"""Processing, stage 3 of the cycle (spec § Processing): raw report → observation.

Rule-based, no language models. Each adapter has a processor
    observations(r, raws) -> list of observation rows
built from the functions below.
"""
import hashlib
import json
import re
import threading
from functools import cache

import httpx

from engine import regions, store

# Script ranges → language guess (Unicode 15 blocks).
SCRIPTS = [("he", re.compile("[֐-׿]")), ("ar", re.compile("[؀-ۿ]")),
           ("ru", re.compile("[Ѐ-ӿ]"))]


def lang(s):
    return next((l for l, p in SCRIPTS if p.search(s or "")), "en")


_lock, _cache = threading.Lock(), None


def _translations():
    global _cache
    if _cache is None:
        p = store.data_dir() / "cache" / "translate.jsonl"
        _cache = {}
        if p.exists():
            for line in p.read_text(encoding="utf8").splitlines():
                k, v = json.loads(line)
                _cache[k] = v
    return _cache


def google(s):
    """Google's unofficial translation endpoint, as upstream (src/lib/hebrew.ts)."""
    res = httpx.get("https://translate.googleapis.com/translate_a/single",
                    params={"client": "gtx", "sl": "auto", "tl": "en", "dt": "t", "q": s},
                    headers={"User-Agent": "Mozilla/5.0"}, timeout=5.0)
    res.raise_for_status()
    return "".join(part[0] for part in res.json()[0] if part[0]) or s


def translate(s):
    """English text of s; s itself if the endpoint fails. Cached on disk."""
    if lang(s) == "en":
        return s
    C = _translations()
    if s in C:
        return C[s]
    try:
        t = google(s)
    except (httpx.HTTPError, ValueError, IndexError, TypeError):
        return s
    with _lock:
        C[s] = t
        p = store.data_dir() / "cache" / "translate.jsonl"
        p.parent.mkdir(parents=True, exist_ok=True)
        with p.open("a", encoding="utf8") as f:
            f.write(json.dumps([s, t], ensure_ascii=False) + "\n")
    return t


COUNTRIES = {
    "iran", "israel", "iraq", "syria", "lebanon", "yemen", "jordan", "egypt", "saudi arabia",
    "qatar", "kuwait", "bahrain", "oman", "uae", "turkey", "ukraine", "russia", "belarus",
    "moldova", "poland", "romania", "gaza", "west bank",
}


GEONAMES = regions.DIR / "geonames.tsv"   # built by `engine gazetteer` (GeoNames cities15000)
WORD = re.compile(r"[^\W\d_][\w'’-]*")
ORDER = {"point": 0, "city": 1, "admin": 2, "country": 3}


@cache
def _geonames():
    if not GEONAMES.exists():
        return []
    rows = [l.split("\t") for l in GEONAMES.read_text(encoding="utf8").splitlines()[1:]]
    return [(n, float(a), float(b), int(pop)) for n, a, b, _, pop in rows]


@cache
def _gazetteer(rid):
    r = regions.regions()[rid]
    L = {k: (lat, lon, "country" if k in COUNTRIES else "city")
         for k, (lat, lon) in r["conflict"]["client"]["strikeLocations"].items()}
    C = {}
    for n, lat, lon, pop in sorted(_geonames(), key=lambda g: g[3]):  # largest wins a name
        if regions.in_bbox(r, lat, lon) and (len(n) >= 5 or pop >= 500_000):
            C[n] = (lat, lon, "city")
    return L, C


def build_geonames(zip_path):
    """Write GEONAMES from GeoNames cities15000.zip (geonames.org export dump, CC BY
    4.0): name and ASCII name of every city inside some region's box. Returns rows."""
    import zipfile
    R = regions.regions().values()
    out = {}
    for l in zipfile.ZipFile(zip_path).read("cities15000.txt").decode("utf8").splitlines():
        f = l.split("\t")   # geonameid name asciiname alternatenames lat lon ... population
        lat, lon = float(f[4]), float(f[5])
        if any(regions.in_bbox(r, lat, lon) for r in R):
            for n in {f[1], f[2]}:
                out[(n, f[8])] = (n, f[4], f[5], f[8], f[14])
    rows = sorted(out.values())
    GEONAMES.write_text("name\tlat\tlon\tcountry\tpopulation\n"
                        + "".join("\t".join(x) + "\n" for x in rows), encoding="utf8")
    _geonames.cache_clear()
    _gazetteer.cache_clear()
    return len(rows)


def gazetteer(r):
    """(L, C): the conflict's curated strikeLocations, matched case-insensitively,
    and GeoNames cities inside the region's box, matched as capitalised words.
    Names under 5 letters need population >= 500,000 ("Hit", "Kut" are words)."""
    return _gazetteer(r["id"])


def geocode(G, s):
    """(lat, lon, precision, names) of the most specific place named in s: the
    finest precision, then the longest name. O(len(s)) phrase lookups."""
    L, C = G
    w, hits = WORD.findall(s), {}
    for n in (3, 2, 1):
        for i in range(len(w) - n + 1):
            g = " ".join(w[i:i + n])
            if g.lower() in L:
                hits[g.lower()] = L[g.lower()]
            elif g in C and g[0].isupper():
                hits[g] = C[g]
    if not hits:
        return None, None, "none", []
    k = min(hits, key=lambda k: (ORDER[hits[k][2]], -len(k)))
    return hits[k][0], hits[k][1], hits[k][2], list(hits)


# Kind rules over English text, first match wins (spec § Event taxonomy).
ATTACK = r"attack|strike|struck|hit|seiz|hijack|missile|drone|explosion|fire"
RULES = [
    ("air_raid_alert", r"air.?raid|sirens?\b|red alert|rocket alert"),
    ("maritime_incident", (rf"(ship|vessel|tanker|cargo|freighter)\w*\W+(?:\w+\W+){{0,6}}({ATTACK})"
                           rf"|({ATTACK})\w*\W+(?:\w+\W+){{0,6}}(ship|vessel|tanker|cargo|freighter)")),
    ("violence_against_civilians", r"civilians? (killed|dead|wounded)|massacre|execut|abduct"),
    ("explosions_remote_violence", (r"air ?strikes?|missiles?|drones?|shell(ing|ed)|bomb|"
                                    r"explosions?|blasts?|rockets?|intercept|struck")),
    ("battles", r"clash|fighting|battle|offensive|assault|advanc|front.?line|firefight"),
    ("riots", r"\briot|\bmob\b|unrest"),
    ("protests", r"protest|demonstrat|\brall(y|ies)\b"),
    ("strategic_developments", r"ceasefire|sanction|talks|negotiat|deploy|agreement|truce"),
]
RULES = [(k, re.compile(p, re.IGNORECASE)) for k, p in RULES]


def rules(s):
    """Kind by the first matching keyword rule (the P0 classifier, now fallback)."""
    return next((k for k, p in RULES if p.search(s)), "report")


def classify(s):
    """Kind of an English text: the trained classifier (engine.classifier), which
    beat the rules 0.68 to 0.54 in weighted accuracy; the rules without sklearn."""
    try:
        from engine.classifier import predict
    except ImportError:
        return rules(s)
    return predict(s)


def credibility_single(r, source_id):
    """Admiralty credibility of a one-observation event (spec § Credibility)."""
    g = regions.reliability(r, source_id)
    return 3 if g in "AB" else 4 if g in "CDE" else 6


def obs_id(raw_id, k=0):
    return hashlib.sha256(f"obs\0{raw_id}\0{k}".encode()).hexdigest()[:32]


def observation(r, x, text):
    """Observation of raw report x whose claim is `text`: translated, geocoded,
    classified, graded, with the event time it states (spec § Processing)."""
    from engine.process.when import event_time
    en = translate(text)
    lat, lon, prec, names = geocode(gazetteer(r), en)
    t = x["published_at"] or x["fetched_at"]
    return {"id": obs_id(x["id"]), "raw_id": x["id"], "source_id": x["source_id"],
            "region": r["id"], "t": t, "lat": lat, "lon": lon,
            "geo_precision": prec, "kind": classify(en), "text_en": en, "lang": lang(text),
            "credibility": credibility_single(r, x["source_id"]), "names": names,
            "t_event": event_time(en, t, r["timezone"])}
