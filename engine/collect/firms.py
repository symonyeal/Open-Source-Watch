"""NASA FIRMS VIIRS (Suomi NPP) active fires: the global 24 h CSV every 10 minutes,
as upstream src/app/api/fires, and the 7-day CSV once a day for the persistence
baseline (engine.process.feeds.firms). Detections inside any region's box are
kept; the 7-day file contributes only detections older than 24 h, so the two
sources do not overlap."""
import csv
import io
from datetime import UTC, datetime, timedelta

from engine import regions

CADENCE = 600
BASE = "https://firms.modaps.eosdis.nasa.gov/data/active_fire/suomi-npp-viirs-c2/csv/"
URL = BASE + "SUOMI_VIIRS_C2_Global_24h.csv"
URL7 = BASE + "SUOMI_VIIRS_C2_Global_7d.csv"
FIELDS = ("latitude", "longitude", "bright_ti4", "frp", "confidence", "acq_date", "acq_time",
          "daynight", "satellite")


def sources(R):
    return [{"id": "firms:VIIRS#7d", "region": "global", "url": URL7, "cadence": 86400},
            {"id": "firms:VIIRS", "region": "global", "url": URL, "cadence": CADENCE}]


def key(p):
    """Identity of one detection, the same in the 24 h and 7-day files."""
    return (f"{float(p['latitude'])},{float(p['longitude'])},{p['acq_date']},"
            f"{p['acq_time'].zfill(4)},{p['satellite']}")


def parse(body, src):
    R = regions.regions()
    old = datetime.now(UTC) - timedelta(hours=24) if src.get("id", "").endswith("#7d") else None
    items = []
    for row in csv.DictReader(io.StringIO(body.decode("utf8"))):
        lat, lon = float(row["latitude"]), float(row["longitude"])
        if not any(regions.in_bbox(r, lat, lon) for r in R.values()):
            continue
        hhmm = row["acq_time"].zfill(4)
        t = datetime.fromisoformat(f"{row['acq_date']}T{hhmm[:2]}:{hhmm[2:]}:00+00:00")
        if old and t >= old:
            continue
        items.append({"native_id": key(row),
                      "published_at": t, "url": None,
                      "payload": {k: row[k] for k in FIELDS}})
    return items

