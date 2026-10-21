"""NASA FIRMS VIIRS (Suomi NPP) active fires, global 24 h CSV, as upstream
src/app/api/fires. Detections inside any region's bounding box are kept."""
import csv
import io
from datetime import datetime

from engine import regions

CADENCE = 600
URL = ("https://firms.modaps.eosdis.nasa.gov/data/active_fire/suomi-npp-viirs-c2/csv/"
       "SUOMI_VIIRS_C2_Global_24h.csv")
FIELDS = ("latitude", "longitude", "bright_ti4", "frp", "confidence", "acq_date", "acq_time",
          "daynight", "satellite")


def sources(R):
    return [{"id": "firms:VIIRS", "region": "global", "url": URL, "cadence": CADENCE}]


def parse(body, src):
    R = regions.regions()
    items = []
    for row in csv.DictReader(io.StringIO(body.decode("utf8"))):
        lat, lon = float(row["latitude"]), float(row["longitude"])
        if not any(regions.in_bbox(r, lat, lon) for r in R.values()):
            continue
        hhmm = row["acq_time"].zfill(4)
        t = datetime.fromisoformat(f"{row['acq_date']}T{hhmm[:2]}:{hhmm[2:]}:00+00:00")
        items.append({"native_id": f"{lat},{lon},{row['acq_date']},{hhmm},{row['satellite']}",
                      "published_at": t, "url": None,
                      "payload": {k: row[k] for k in FIELDS}})
    return items

