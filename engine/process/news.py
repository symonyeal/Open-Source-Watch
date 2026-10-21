"""News raw reports (rss:*, gnews:*) → observations, with upstream's relevance
rule (src/app/api/news/route.ts): sports/entertainment noise is dropped; items
from an unfiltered feed or a Google News query pass; otherwise the title or
category must match the theater's relevance keywords. Irrelevant reports stay in
raw_report with no observation."""
import json
import re

from engine import regions
from engine.collect import rss
from engine.process import observation

NOISE = re.compile(r"world.?cup|\bfifa\b|\bioc\b|olympic|premier.?league|champions.?league|"
                   r"super.?bowl|\bnba\b|\bnfl\b|\bnhl\b|\bmlb\b|grammy|oscar|\bemmy|"
                   r"box.?office|celebrity|eurovision", re.IGNORECASE)


def relevant(title, category, unfiltered, K):
    if NOISE.search(title):
        return False
    return unfiltered or bool(K.search(title) or K.search(category or ""))


def derive(raws, R):
    out = []
    for x in raws:
        r, p = R[x["region"]], json.loads(x["payload"])
        F = rss.feeds(r)
        unfiltered = x["source_id"].startswith("gnews:") or \
            F.get(x["source_id"], {}).get("unfiltered", False)
        if relevant(p["title"], p.get("category"), unfiltered,
                    regions.regex(r["conflict"]["newsRelevanceKeywords"])):
            out.append(observation(r, x, p["title"]))
    return {"observation": out}
