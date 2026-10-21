"""Processing, stage 3 of the cycle (spec § Processing): raw report → observation.

Rule-based, no language models. Each adapter has a processor
    observations(r, raws) -> list of observation rows
built from the functions below.
"""
import hashlib
import json
import re
import threading

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


def translate(s):
    """English text of s via Google's unofficial endpoint, as upstream
    (src/lib/hebrew.ts translateFreeText); s itself on failure. Cached on disk."""
    if lang(s) == "en":
        return s
    C = _translations()
    if s in C:
        return C[s]
    try:
        res = httpx.get("https://translate.googleapis.com/translate_a/single",
                        params={"client": "gtx", "sl": "auto", "tl": "en", "dt": "t", "q": s},
                        headers={"User-Agent": "Mozilla/5.0"}, timeout=5.0)
        res.raise_for_status()
        t = "".join(part[0] for part in res.json()[0] if part[0]) or s
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


def gazetteer(r):
    """[(pattern, name, lat, lon, precision)], longest name first. Source: the
    conflict's strikeLocations table (P0); GeoNames replaces it in P1."""
    G = r["conflict"]["client"]["strikeLocations"]
    return [(re.compile(rf"\b{re.escape(k)}\b", re.IGNORECASE), k, lat, lon,
             "country" if k in COUNTRIES else "city")
            for k, (lat, lon) in sorted(G.items(), key=lambda kv: -len(kv[0]))]


def geocode(G, s):
    """(lat, lon, precision, names) of the most specific gazetteer match in s."""
    hits = [g for g in G if g[0].search(s)]
    if not hits:
        return None, None, "none", []
    order = {"point": 0, "city": 1, "admin": 2, "country": 3}
    _, _, lat, lon, prec = min(hits, key=lambda g: (order[g[4]], -len(g[1])))
    return lat, lon, prec, [g[1] for g in hits]


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


def classify(s):
    return next((k for k, p in RULES if p.search(s)), "report")


def credibility_single(r, source_id):
    """Admiralty credibility of a one-observation event (spec § Credibility)."""
    g = regions.reliability(r, source_id)
    return 3 if g in "AB" else 4 if g in "CDE" else 6


def obs_id(raw_id, k=0):
    return hashlib.sha256(f"obs\0{raw_id}\0{k}".encode()).hexdigest()[:32]
