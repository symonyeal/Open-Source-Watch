"""UCDP Candidate Events Dataset, monthly release (docs/SOURCES.md).

UCDP codes each incident of organized lethal violence (state-based, non-state,
one-sided; at least one direct death) with dates, coordinates and their
precision (Candidate codebook 1.5). The monthly file `GEDEvent_v<yy>_0_<m>.csv`,
linked from the downloads page, needs no key (the API needs a token). Each poll
reads the page and, unless that version is stored, its newest monthly file. One
raw report is one event inside a region's box, native id "<id>@<version>": the
id persists across versions, so a revised event is a new report and readers keep
the latest version. These events are the reference that engine.validate scores
the engine's events against, so they never become observations.
"""
import csv
import io
import json
import re
from datetime import UTC, datetime
from functools import lru_cache
from types import SimpleNamespace

from engine import regions, store
from engine.collect import EXCERPT
from engine.collect import fetch as get

PAGE = "https://ucdp.uu.se/downloads/"
CADENCE = 7 * 86400
FIELDS = ("id", "type_of_violence", "conflict_name", "side_a", "side_b", "country",
          "date_start", "date_end", "date_prec", "where_prec", "latitude", "longitude",
          "best", "low", "high", "code_status", "event_clarity", "where_description")
INTS = ("type_of_violence", "date_prec", "where_prec", "best", "low", "high", "event_clarity")


def sources(R):
    return [{"id": "ucdp:UCDP Candidate", "region": rid, "url": PAGE, "cadence": CADENCE}
            for rid in R]


def fetch(src):
    """{"version", "url", "csv"} of the newest monthly file; csv is empty if that
    version is already stored."""
    L = re.findall(r'(?i)href="(https://ucdp\.uu\.se/downloads/candidateged/'
                   r'GEDEvent_v(\d+)_0_(\d+)\.csv)"', get(src).text)
    if not L:
        raise ValueError("No monthly UCDP Candidate file linked")
    url, yy, m = max(L, key=lambda x: (int(x[1]), int(x[2])))
    v = f"{yy}.0.{m}"
    have = store.query("SELECT 1 FROM raw_report WHERE source_id LIKE 'ucdp:%' AND "
                       "json_extract_string(payload, '$.version') = ? LIMIT 1", (v,))
    text = "" if have else get({"url": url}, timeout=120.0).content.decode("utf-8-sig")
    return SimpleNamespace(content=json.dumps({"version": v, "url": url, "csv": text}).encode())


@lru_cache(maxsize=4)
def rows(text):
    return list(csv.DictReader(io.StringIO(text)))


def parse(body, src):
    d, r = json.loads(body), regions.regions()[src["region"]]
    out = []
    for x in rows(d["csv"]):
        lat, lon = float(x["latitude"]), float(x["longitude"])
        if not regions.in_bbox(r, lat, lon):
            continue
        p = {k: x[k] for k in FIELDS} | {k: int(x[k]) for k in INTS} | {
            "latitude": lat, "longitude": lon, "version": d["version"],
            "date_start": x["date_start"][:10], "date_end": x["date_end"][:10],   # dates only
            "where_description": x["where_description"][:EXCERPT]}
        out.append({"native_id": f"{x['id']}@{d['version']}", "url": d["url"], "payload": p,
                    "published_at": datetime.fromisoformat(x["date_start"]).replace(tzinfo=UTC)})
    return out
