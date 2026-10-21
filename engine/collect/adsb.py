"""adsb.lol aircraft, as upstream src/app/api/flights: the global military feed
and each region's radius query. Kept: military feed aircraft inside a region's
bounding box, and radius aircraft with dbFlags military (1) or interesting (2).
Upstream also shows aircraft it classifies by callsign or type alone; the engine
does not store those (docs/DECISIONS.md)."""
import json
from datetime import UTC, datetime, timedelta

from engine import regions

CADENCE = 30
FIELDS = ("hex", "flight", "t", "r", "desc", "ownOp", "lat", "lon", "alt_baro", "gs", "track",
          "squawk", "dbFlags")


def sources(R):
    S = [{"id": "adsb:adsb.lol#mil", "region": "global", "url": "https://api.adsb.lol/v2/mil",
          "cadence": CADENCE, "headers": {"Accept": "application/json"}}]
    for rid, r in R.items():
        c = r["conflict"]["flightsCenter"]
        S.append({"id": f"adsb:adsb.lol#{rid}", "region": rid, "cadence": CADENCE,
                  "url": f"https://api.adsb.lol/v2/lat/{c['lat']}/lon/{c['lon']}/dist/{c['dist']}",
                  "headers": {"Accept": "application/json"}})
    return S


def parse(body, src):
    d = json.loads(body)
    t0 = datetime.fromtimestamp(d["now"] / 1000, UTC) if "now" in d else datetime.now(UTC)
    R = regions.regions()
    items = []
    for a in d.get("ac") or []:
        lat, lon = a.get("lat"), a.get("lon")
        if lat is None or lon is None:
            continue
        if src["region"] == "global":
            if not any(regions.in_bbox(r, lat, lon) for r in R.values()):
                continue
        elif not (a.get("dbFlags") or 0) & 3:
            continue
        t = t0 - timedelta(seconds=a.get("seen_pos") or 0)
        hexid = (a.get("hex") or "").strip().replace("~", "")
        items.append({"native_id": f"{hexid}@{int(t.timestamp())}", "published_at": t,
                      "url": None, "payload": {k: a[k] for k in FIELDS if k in a}})
    return items
