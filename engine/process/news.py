"""News raw reports (rss:*, gnews:*) → observations, with upstream's relevance
rule (src/app/api/news/route.ts): sports/entertainment noise is dropped; items
from an unfiltered feed or a Google News query pass; otherwise the title or
category must match the theater's relevance keywords. Irrelevant reports stay in
raw_report with no observation."""
import json
import re

from engine import regions
from engine.collect import rss
from engine.process import observations

NOISE = re.compile(r"world.?cup|\bfifa\b|\bioc\b|olympic|premier.?league|champions.?league|"
                   r"super.?bowl|\bnba\b|\bnfl\b|\bnhl\b|\bmlb\b|grammy|oscar|\bemmy|"
                   r"box.?office|celebrity|eurovision", re.IGNORECASE)
REGIONAL_NOISE = re.compile(
    r"\bcricket|\bcricketer|\bipl\b|\bfootball\b|\bsoccer\b|\bsport(s)?\b|"
    r"\bfilm\b|\bmovie\b|\bcinema\b|\bactor\b|\bactress\b|\bbollywood\b|"
    r"\bhollywood\b|\btollywood\b|\bbox.?office\b|\btrailer\b|\bcelebrity\b|"
    r"\bentertainment\b|\bstreaming\b|\bnetflix\b|\bkartik aaryan\b|"
    r"\bworld.?cup\b|\bchampions.?league\b|\bpremier.?league\b|\bnba\b|\bnfl\b|"
    r"\bnhl\b|\bmlb\b|\bgrammy\b|\boscar\b|\bemmy\b|\beurovision\b",
    re.IGNORECASE,
)
COUNTRY_ALIASES = {
    "bangladesh": ("Bangladesh", "Bangladeshi"),
    "india": ("India", "Indian"),
    "pakistan": ("Pakistan", "Pakistani"),
    "sri lanka": ("Sri Lanka", "Sri Lankan", "Ceylon", "Ceylonese"),
    "myanmar": ("Myanmar", "Burmese", "Burma"),
    "nepal": ("Nepal", "Nepali", "Nepalese"),
    "bhutan": ("Bhutan", "Bhutanese"),
    "maldives": ("Maldives", "Maldivian"),
}


def relevant(title, category, unfiltered, K):
    if NOISE.search(title):
        return False
    return unfiltered or bool(K.search(title) or K.search(category or ""))


def regional_google_news(title, source_id, K, countries):
    """South Asia Google News needs an in-title geographic match; country feeds
    also need their own country or demonym in the headline. `countries` is the
    region's configured list of {country, query} records.
    """
    if NOISE.search(title) or REGIONAL_NOISE.search(title) or not K.search(title):
        return False
    if not source_id.startswith("gnews:country#"):
        return True

    name = source_id.split("#", 1)[1].casefold()
    country = next((x["country"] for x in countries if x["country"].casefold() == name), None)
    if country is None:
        return False
    aliases = COUNTRY_ALIASES.get(country.casefold(), (country,))
    pattern = re.compile(r"(?<!\w)(?:" + "|".join(map(re.escape, aliases)) + r")(?!\w)",
                         re.IGNORECASE)
    return bool(pattern.search(title))


def derive(raws, R):
    out = []
    for x in raws:
        r, p = R[x["region"]], json.loads(x["payload"])
        F = rss.feeds(r)
        unfiltered = x["source_id"].startswith("gnews:") or \
            F.get(x["source_id"], {}).get("unfiltered", False)
        K = regions.regex(r["conflict"]["newsRelevanceKeywords"])
        accepted = (regional_google_news(p["title"], x["source_id"], K,
                                         r["conflict"]["countryQueries"])
                    if r["conflict"].get("filterGoogleNews") and
                    x["source_id"].startswith("gnews:")
                    else relevant(p["title"], p.get("category"), unfiltered, K))
        if accepted:
            out.append((r, x, p["title"]))
    return {"observation": observations(out)}
