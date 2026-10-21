"""Drone and missile tracks from neptun.in.ua, as upstream src/app/api/drones,
for regions with droneProvider "neptun". An item is one track position."""
import json

from engine.collect import EXCERPT
from engine.collect.rss import date

CADENCE = 30
URL = "https://neptun.in.ua/api/data"
FIELDS = ("threat_type", "place", "region", "count", "course_bearing", "confidence_0_100")


def sources(R):
    return [{"id": "drones:neptun", "region": rid, "url": URL, "cadence": CADENCE,
             "headers": {"Accept": "application/json"}}
            for rid, r in R.items() if r["conflict"].get("droneProvider") == "neptun"]


def parse(body, src):
    d = json.loads(body)
    M = d.get("markers") or d.get("tracks") or []
    items = []
    for m in M:
        if not isinstance(m.get("lat"), (int, float)) or not isinstance(m.get("lng"), (int, float)):
            continue
        tid = m.get("id") or m.get("track_id") or f"{m['lat']},{m['lng']}"
        items.append({"native_id": f"{tid}@{m.get('date')}@{m['lat']:.5f},{m['lng']:.5f}",
                      "published_at": date(m.get("date")), "url": None,
                      "payload": {"track": tid, "lat": m["lat"], "lon": m["lng"],
                                  "text": (m.get("text") or "")[:EXCERPT],
                                  **{k: m.get(k) for k in FIELDS}}})
    return items
