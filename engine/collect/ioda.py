"""IODA (Internet Outage Detection and Analysis, Georgia Tech) outage events per
country (docs/SOURCES.md).

IODA detects country-level connectivity drops in three measurements, each its
own `datasource`: BGP announcements, active probing of /24 blocks (ping-slash24)
and unsolicited traffic at a network telescope (merit-nt). `/v2/outages/events`
returns each detected outage with its start, duration and score. One raw report
is one event as listed; an ongoing event lengthens, so its native id carries the
duration and each new length is a new report (the view keeps the longest).
The countries of a region are its `[outages]` table in region.toml. The merged
`overall=true` events answered HTTP 500 when inspected (2026-10-29).
"""
import json
import time

from engine.collect import epoch, stored
from engine.collect import fetch as get

API = "https://api.ioda.inetintel.cc.gatech.edu/v2/outages/events"
DAYS = 7
CADENCE = 21600


def sources(R):
    return [{"id": f"ioda:IODA#{c['code']}", "region": rid, "code": c["code"],
             "url": f"{API}?entityType=country&entityCode={c['code']}", "cadence": CADENCE}
            for rid, r in R.items() for c in r.get("outages", {}).get("countries", [])]


def fetch(src):
    """The events of the last DAYS; the window is set at fetch time. IODA answers
    in about ten seconds, so the timeout is longer than the default."""
    t = int(time.time())
    return get({"url": f"{src['url']}&from={t - DAYS * 86400}&until={t}"}, timeout=60.0)


def parse(body, src):
    d = json.loads(body)
    if d.get("error") or not isinstance(d.get("data"), list):
        raise ValueError(f"IODA error: {d.get('error')}")
    loc = f"country/{src['code']}"
    return [{"native_id": f"{e['datasource']}:{e['start']}:{e['duration']}",
             "published_at": epoch(1000 * e["start"]),
             "url": f"https://ioda.inetintel.cc.gatech.edu/country/{src['code']}"
                    f"?from={e['start']}&until={e['start'] + e['duration']}",
             "payload": {"country": src["code"], "name": e.get("location_name"),
                         "datasource": e["datasource"], "method": e.get("method"),
                         "start": e["start"], "duration": e["duration"],
                         "score": e.get("score")}}
            for e in d["data"] if e.get("location") == loc]


def events(r, since):
    """Payloads, with url and fetched_at, of region r's IODA events starting at or
    after `since`, one per (country, datasource, start) at its longest listing."""
    E = {}
    for x in stored("ioda", regions=[r["id"]], since=since):
        p = x["payload"]
        k = (p["country"], p["datasource"], p["start"])
        if k not in E or p["duration"] > E[k]["duration"]:
            E[k] = p | {"url": x["url"], "fetched_at": x["fetched_at"]}
    return list(E.values())
