"""UCDP Candidate Events Dataset, monthly release (docs/SOURCES.md).

UCDP codes each incident of organized lethal violence (state-based, non-state,
one-sided; at least one direct death) with dates, coordinates and their
precision (Candidate codebook 1.5). The monthly file `GEDEvent_v<yy>_0_<m>.csv`,
linked from the downloads page, needs no key (the API needs a token). Each poll
reads the page and its newest monthly file; a region that already stores that
version takes nothing from it. One raw report is one event inside a region's
box, native id "<id>@<version>": the id persists across versions, so a revised
event is a new report and `events` keeps the latest version. These events are
the reference that engine.validate scores the engine's events against, so they
never become observations.
"""
import csv
import io
import json
import re
from datetime import UTC, datetime
from functools import lru_cache

from engine import regions
from engine.collect import EXCERPT, response, stored
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
    """{"version", "url", "csv", "have"} of the newest monthly file, `have` the
    regions that store that version. The file is read on every poll: a region whose
    box holds no event of the version stores nothing that would show it was read."""
    L = re.findall(r'(?i)href="(https://ucdp\.uu\.se/downloads/candidateged/'
                   r'GEDEvent_v(\d+)_0_(\d+)\.csv)"', get(src).text)
    if not L:
        raise ValueError("No monthly UCDP Candidate file linked")
    url, yy, m = max(L, key=lambda x: (int(x[1]), int(x[2])))
    v = f"{yy}.0.{m}"
    have = sorted({x["region"] for x in stored("ucdp") if x["payload"]["version"] == v})
    return response({"version": v, "url": url, "have": have,
                     "csv": get({"url": url}, timeout=120.0).content.decode("utf-8-sig")})


@lru_cache(maxsize=4)
def rows(text):
    return list(csv.DictReader(io.StringIO(text)))


def parse(body, src):
    d, r = json.loads(body), regions.regions()[src["region"]]
    if r["id"] in d["have"]:
        return []
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


def events(r):
    """Payloads of region r's stored UCDP events, the latest version of each."""
    v = lambda p: tuple(int(x) for x in p["version"].split("."))
    out = {}
    for x in stored("ucdp", regions=[r["id"]]):
        p = x["payload"]
        if p["id"] not in out or v(p) > v(out[p["id"]]):
            out[p["id"]] = p
    return list(out.values())
