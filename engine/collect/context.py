"""Recorded public context: EIA spot via FRED, IMF PortWatch and World Bank WDI.

One immutable raw report is a bounded series snapshot. Its content hash changes
on revisions; the context view selects the latest fetched snapshot, not a union
of incompatible releases. These measurements never enter the event classifier.
"""
import csv
import hashlib
import io
import json
import math
from datetime import UTC, datetime, timedelta
from urllib.parse import urlencode

BASE = "https://services9.arcgis.com/weJ1QsnbMYJlCHdG/arcgis/rest/services/"
FRED = {"DCOILBRENTEU": "Brent spot", "DCOILWTICO": "WTI spot"}
WDI = {"EG.IMP.CONS.ZS": "Net energy imports (% of energy use)",
       "TM.VAL.FUEL.ZS.UN": "Fuel imports (% of merchandise imports)"}
CHOKES = {"chokepoint1": "Suez Canal", "chokepoint4": "Bab el-Mandeb Strait",
          "chokepoint5": "Malacca Strait", "chokepoint6": "Strait of Hormuz"}


def sources(R):
    start = (datetime.now(UTC).date() - timedelta(days=90)).isoformat()
    S = [{"id": f"context:FRED#{code}", "region": "global", "kind": "fred", "code": code,
          "name": name, "cadence": 21600, "url": "https://fred.stlouisfed.org/graph/fredgraph.csv?"
          + urlencode({"id": code, "cosd": start, "coed": datetime.now(UTC).date().isoformat()})}
         for code, name in FRED.items()]
    for rid, r in R.items():
        cfg = r.get("context", {})
        countries = cfg.get("countries", [])
        if countries:
            for code, name in WDI.items():
                S.append({"id": f"context:World Bank#{code}", "region": rid, "kind": "wdi",
                          "code": code, "name": name, "countries": countries, "cadence": 86400,
                          "url": f"https://api.worldbank.org/v2/country/{';'.join(countries)}"
                          f"/indicator/{code}?format=json&date=2010:"
                          f"{datetime.now(UTC).year}&per_page=1000"})
        for port in cfg.get("ports", []):
            S.append(port_source(port["id"], port["name"], rid, "port", start))
    S += [port_source(code, name, "global", "chokepoint", start)
          for code, name in CHOKES.items()]
    return S


def port_source(code, name, region, kind, start):
    table = "Daily_Ports_Data" if kind == "port" else "Daily_Chokepoints_Data"
    fields = "date,portid,portname," + ("portcalls,portcalls_tanker" if kind == "port"
                                       else "n_total,n_tanker")
    # Only registry identifiers, never user input, enter ArcGIS SQL.
    params = {"where": f"portid='{code}' AND date >= DATE '{start}'", "outFields": fields,
              "orderByFields": "date ASC", "resultRecordCount": 1000,
              "returnGeometry": "false", "f": "json"}
    return {"id": f"context:PortWatch#{code}", "region": region, "kind": kind, "code": code,
            "name": name, "cadence": 21600,
            "url": BASE + table + "/FeatureServer/0/query?" + urlencode(params)}


def number(x):
    """Missing stays missing; malformed or non-finite measures fail the poll."""
    if x is None or x in ("", "."):
        return None
    if isinstance(x, bool):
        raise TypeError("Boolean measure")
    v = float(x)
    if not math.isfinite(v):
        raise ValueError("Non-finite measure")
    return v


def day(x):
    """ArcGIS supports both legacy epoch milliseconds and ISO date-only values."""
    if isinstance(x, (int, float)):
        return datetime.fromtimestamp(x / 1000, UTC).date().isoformat()
    return datetime.fromisoformat(x).date().isoformat()


def parse(body, src):
    kind, points = src["kind"], []
    if kind == "fred":
        for row in csv.DictReader(io.StringIO(body.decode("utf-8-sig"))):
            points.append({"date": day(row.get("observation_date") or row["DATE"]),
                           "value": number(row[src["code"]])})
        unit, credit = "USD/bbl", "U.S. EIA via FRED"
        url = "https://fred.stlouisfed.org/series/" + src["code"]
    elif kind == "wdi":
        d = json.loads(body)
        if not isinstance(d, list) or len(d) != 2 or int(d[0]["pages"]) > 1:
            raise ValueError("World Bank error or incomplete page")
        for row in d[1] or []:
            if row["countryiso3code"] in src["countries"]:
                points.append({"country": row["countryiso3code"], "name": row["country"]["value"],
                               "date": str(row["date"]), "value": number(row["value"])})
        unit, credit = "%", "World Bank WDI (IEA / UN Comtrade)"
        url = "https://data.worldbank.org/indicator/" + src["code"]
    else:
        d = json.loads(body)
        if "error" in d or d.get("exceededTransferLimit"):
            raise ValueError("PortWatch error or incomplete page")
        total, tanker = ("portcalls", "portcalls_tanker") if kind == "port" else (
            "n_total", "n_tanker")
        for feature in d["features"]:
            row = feature["attributes"]
            if row["portid"] != src["code"]:
                raise ValueError("Unexpected PortWatch location")
            points.append({"date": day(row["date"]), "value": number(row[total]),
                           "tanker": number(row[tanker])})
        unit = "calls/day" if kind == "port" else "transits/day"
        credit, url = "UN Global Platform; IMF PortWatch", "https://portwatch.imf.org/"
    if not points:
        return []
    points.sort(key=lambda p: (p.get("country", ""), p["date"]))
    payload = {"code": src["code"], "name": src["name"], "kind": kind,
               "unit": unit, "source": credit, "source_url": url, "points": points}
    digest = hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()[:24]
    t = max(p["date"] for p in points)
    t = datetime.fromisoformat(t + ("-01-01" if len(t) == 4 else "")).replace(tzinfo=UTC)
    # Keep release order even when a provider reverts to an earlier value set.
    return [{"native_id": f"{digest}@{datetime.now(UTC).isoformat()}", "published_at": t,
             "url": url, "payload": payload}]
