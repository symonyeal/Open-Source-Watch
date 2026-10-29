"""Navigation-integrity reports of aircraft in each region's adsb.lol radius
query, the response engine.collect.adsb already fetches (docs/SOURCES.md).

ADS-B version 1/2 aircraft report how far their position can be trusted: NIC
(containment radius) and NACp (position accuracy); readsb adds `gpsOkBefore`
while GPS is lost. One raw report is one region's snapshot every CADENCE
seconds: [icao, lat, lon, nic, nacp, lost] per airborne aircraft in the box.
The validity filters transcribe gods-eye-view `src/layers/gnss/records.js`;
whether a report is bad is decided later (engine.gnss), so the rule can change
without recollection. A local engine only: the cron does not poll aircraft.
"""
import json
import math

from engine import regions
from engine.collect import adsb, epoch

CADENCE = 600
TYPES = ("adsb_icao", "mlat")   # after GPS loss the position source can switch to MLAT
VERSIONS = (1, 2)               # DO-260A/B quality fields
AGE = 60                        # s; an older position is not current evidence


def sources(R):
    return [s | {"id": s["id"].replace("adsb:", "integrity:", 1), "cadence": CADENCE}
            for s in adsb.sources(R) if s["region"] != "global"]


def finite(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


def parse(body, src):
    d = json.loads(body)
    r = regions.regions()[src["region"]]
    out = []
    for a in d.get("ac") or []:
        age = a.get("seen_pos")
        if a.get("type") not in TYPES or a.get("version") not in VERSIONS \
                or not finite(a.get("alt_baro")) or not (finite(age) and 0 <= age <= AGE):
            continue
        derived = set(a.get("tisb") or []) | set(a.get("mlat") or [])
        nic = a["nic"] if finite(a.get("nic")) and "nic" not in derived else None
        nacp = a["nac_p"] if finite(a.get("nac_p")) and "nac_p" not in derived else None
        lost = finite(a.get("gpsOkBefore")) and a["gpsOkBefore"] > 0
        lat, lon = a.get("lat"), a.get("lon")
        if lost and finite(a.get("gpsOkLat")) and finite(a.get("gpsOkLon")):
            lat, lon = a["gpsOkLat"], a["gpsOkLon"]   # the last good GNSS position
        if (nic is None and nacp is None and not lost) or not (finite(lat) and finite(lon)) \
                or not regions.in_bbox(r, lat, lon):
            continue
        out.append([a["hex"].strip(), round(lat, 4), round(lon, 4), nic, nacp, lost])
    if not out:
        return []
    t = epoch(d["now"])
    return [{"native_id": t.isoformat(), "published_at": t, "url": None,
             "payload": {"ac": out}}]
