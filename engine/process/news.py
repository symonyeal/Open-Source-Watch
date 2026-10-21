"""News raw reports → observations, with upstream's relevance rule
(src/app/api/news/route.ts): sports/entertainment noise is dropped; items from an
unfiltered feed pass; otherwise the title or category must match the theater's
relevance keywords. Irrelevant reports stay in raw_report with no observation."""
import json
import re

from engine import regions
from engine.collect import rss
from engine.process import (classify, credibility_single, gazetteer, geocode, lang, obs_id,
                            translate)

NOISE = re.compile(r"world.?cup|\bfifa\b|\bioc\b|olympic|premier.?league|champions.?league|"
                   r"super.?bowl|\bnba\b|\bnfl\b|\bnhl\b|\bmlb\b|grammy|oscar|\bemmy|"
                   r"box.?office|celebrity|eurovision", re.IGNORECASE)


def relevant(title, category, unfiltered, K):
    if NOISE.search(title):
        return False
    return unfiltered or bool(K.search(title) or K.search(category or ""))


def observations(r, raws):
    F, K, G = rss.feeds(r), regions.regex(r["conflict"]["newsRelevanceKeywords"]), gazetteer(r)
    out = []
    for x in raws:
        p = json.loads(x["payload"])
        if not relevant(p["title"], p.get("category"), F.get(x["source_id"], {}).get("unfiltered"),
                        K):
            continue
        en = translate(p["title"])
        lat, lon, prec, names = geocode(G, en)
        out.append({
            "id": obs_id(x["id"]), "raw_id": x["id"], "source_id": x["source_id"],
            "region": x["region"], "t": x["published_at"] or x["fetched_at"],
            "lat": lat, "lon": lon, "geo_precision": prec, "kind": classify(en), "text_en": en,
            "lang": lang(p["title"]), "credibility": credibility_single(r, x["source_id"]),
            "names": names,
        })
    return out
