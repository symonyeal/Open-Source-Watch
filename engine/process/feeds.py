"""Processors of the non-news feeds: raw reports → observations and track points
(spec § Processing). One function per adapter, `derive(raws, R) -> {table: rows}`, of
raw reports with decoded payloads (engine.collect.decode); `entities` merges the
aircraft and vessels their reports describe."""
import json
from collections import defaultdict
from datetime import date, timedelta
from functools import cache
from pathlib import Path

from engine import regions
from engine.collect import stored
from engine.collect.firms import key as firms_key
from engine.process import credibility_single, lang, obs_id, observations, translate

CITY = Path(__file__).resolve().parents[2] / "src" / "data" / "city-data.json"


def telegram(raws, R):
    return {"observation": observations(
        [(R[x["region"]], x, x["payload"]["text"]) for x in raws])}


CELL, PRIOR, PERSIST = 0.1, 7, 3   # degrees; days; active days that make a cell persistent


def cell(lat, lon):
    return round(lat / CELL), round(lon / CELL)


def persistent(D, c, d):
    """A detection in cell c on day d is at a persistent heat source (gas flare,
    industry) if c had detections on >= PERSIST of the PRIOR days before d; D maps
    cell -> set of detection dates. Static-source masking as in FIRMS / VIIRS
    Nightfire flare catalogues (docs/SOURCES.md)."""
    return sum(1 for k in range(1, PRIOR + 1) if d - timedelta(days=k) in D.get(c, ())) >= PERSIST


def firms(raws, R):
    """One observation per detection per region containing it: thermal_anomaly,
    or report at a persistent heat source. Low-confidence ('l') detections are
    not observations. History comes from the immutable raw layer, so the result
    is a function of raw reports."""
    if not raws:
        return {"observation": []}
    t0 = min(x["published_at"] for x in raws) - timedelta(days=PRIOR + 1)
    D = defaultdict(set)
    for p in [x["payload"] for x in stored("firms", since=t0) + raws]:
        D[cell(float(p["latitude"]), float(p["longitude"]))].add(date.fromisoformat(p["acq_date"]))
    out = []
    for x in raws:
        p = x["payload"]
        lat, lon = float(p["latitude"]), float(p["longitude"])
        if p["confidence"] == "l":
            continue
        hot = persistent(D, cell(lat, lon), date.fromisoformat(p["acq_date"]))
        for r in R.values():
            if regions.in_bbox(r, lat, lon):
                out.append({
                    "id": obs_id(firms_key(p), r["id"]), "raw_id": x["id"],
                    "source_id": x["source_id"], "region": r["id"], "t": x["published_at"],
                    "lat": lat, "lon": lon, "geo_precision": "point",
                    "kind": "report" if hot else "thermal_anomaly",
                    "text_en": f"VIIRS {'persistent heat source' if hot else 'thermal anomaly'}, "
                               f"FRP {p['frp']} MW, confidence {p['confidence']}, {p['daynight']}",
                    "lang": "en", "credibility": credibility_single(r, x["source_id"]),
                    "names": [], "t_event": x["published_at"]})
    return {"observation": out}


def ais(raws, R):
    """Track points of vessels, entity "mmsi:<mmsi>", in each region containing them."""
    out = []
    for x in raws:
        p = x["payload"]
        out += [{"entity_id": f"mmsi:{p['mmsi']}", "region": r["id"], "t": x["published_at"],
                 "lat": p["lat"], "lon": p["lon"], "heading": p.get("cog"), "speed": p.get("sog"),
                 "source_id": x["source_id"]}
                for r in R.values() if regions.in_bbox(r, p["lat"], p["lon"])]
    return {"track_point": out}


def adsb(raws, R):
    """Track points of aircraft, entity "icao:<hex>", in each region containing them."""
    out = []
    for x in raws:
        p = x["payload"]
        for r in R.values():
            if regions.in_bbox(r, p["lat"], p["lon"]):
                out.append({"entity_id": "icao:" + p["hex"].strip().replace("~", ""),
                            "region": r["id"], "t": x["published_at"], "lat": p["lat"],
                            "lon": p["lon"], "heading": p.get("track"), "speed": p.get("gs"),
                            "source_id": x["source_id"]})
    return {"track_point": out}


@cache
def hebrew():
    """Hebrew → English: upstream's alert vocabulary and CBS locality names."""
    g = json.loads((regions.DIR / "global.json").read_text(encoding="utf8"))
    return g["threatTranslations"], json.loads(CITY.read_text(encoding="utf8"))


def alerts(raws, R):
    """One air_raid_alert per alert, placed at its first location in alertCities."""
    T, C = hebrew()
    out = []
    for x in raws:
        r, p = R[x["region"]], x["payload"]
        threat = T.get(p["threat"], p["threat"])
        places = [C.get(s.strip(), s.strip()) for s in p["locations"]]
        en = translate(f"{threat}: {', '.join(places)}")
        A = r["conflict"]["client"]["alertCities"]
        hit = next((s.lower() for s in places if s.lower() in A), None)
        lat, lon = A[hit] if hit else (None, None)
        prec = ("admin" if "oblast" in hit else "city") if hit else "none"
        out.append({"id": obs_id(x["id"]), "raw_id": x["id"], "source_id": x["source_id"],
                    "region": r["id"], "t": x["published_at"] or x["fetched_at"], "lat": lat,
                    "lon": lon, "geo_precision": prec, "kind": "air_raid_alert", "text_en": en,
                    "lang": lang(p["threat"]),
                    "credibility": credibility_single(r, x["source_id"]), "names": places,
                    "t_event": x["published_at"] or x["fetched_at"]})
    return {"observation": out}


def ioda(raws, R):
    """One internet_outage per IODA event, placed at the seat of its country's
    government (region.toml [outages]) with admin precision: IODA measures the
    country as one unit, so outages of one country within τ of each other link
    under the fusion rule (same unit). The id depends on the event's country,
    datasource and start only, so a lengthening event stays one observation."""
    out = []
    for x in raws:
        r, p = R[x["region"]], x["payload"]
        c = next(c for c in r["outages"]["countries"] if c["code"] == p["country"])
        h = p["duration"] / 3600
        out.append({"id": obs_id(f"ioda:{p['country']}:{p['datasource']}:{p['start']}", r["id"]),
                    "raw_id": x["id"], "source_id": x["source_id"], "region": r["id"],
                    "t": x["published_at"], "lat": c["lat"], "lon": c["lon"],
                    "geo_precision": "admin", "kind": "internet_outage",
                    "text_en": f"IODA {p['datasource']} outage signal in {c['name']} from "
                               f"{x['published_at']:%Y-%m-%d %H:%M} UTC, {h:.1f} h listed"
                               f" (score {p['score'] or 0:.0f})",
                    "lang": "en", "credibility": credibility_single(r, x["source_id"]),
                    "names": [c["name"]], "t_event": x["published_at"]})
    return {"observation": out}


def neptun(raws, R):
    """Track points of each drone/missile track, and one airspace_activity
    observation per track (the observation id depends on the track id only)."""
    T, O = [], []
    for x in raws:
        r, p = R[x["region"]], x["payload"]
        T.append({"entity_id": f"neptun:{p['track']}", "region": r["id"],
                  "t": x["published_at"] or x["fetched_at"], "lat": p["lat"], "lon": p["lon"],
                  "heading": p.get("course_bearing"), "speed": None,
                  "source_id": x["source_id"]})
        O.append({"id": obs_id(f"neptun:{p['track']}"), "raw_id": x["id"],
                  "source_id": x["source_id"], "region": r["id"],
                  "t": x["published_at"] or x["fetched_at"], "lat": p["lat"], "lon": p["lon"],
                  "geo_precision": "point", "kind": "airspace_activity",
                  "text_en": translate(f"{p.get('threat_type')}: {p['text']}"),
                  "lang": lang(p["text"]), "credibility": credibility_single(r, x["source_id"]),
                  "names": [p["place"]] if p.get("place") else [],
                  "t_event": x["published_at"] or x["fetched_at"]})
    return {"track_point": T, "observation": O}


def entities(raws):
    """Aircraft (ADS-B, Neptun) and vessel (AIS) entities of decoded raw reports,
    latest attributes win."""
    E = {}
    for x in sorted(raws, key=lambda x: x["fetched_at"]):
        p = x["payload"]
        if x["source_id"].startswith("adsb:"):
            h = p["hex"].strip().replace("~", "")
            e = E.setdefault(f"icao:{h}", {"id": f"icao:{h}", "type": "aircraft", "names": set(),
                                           "identifiers": {"icao": h}, "attributes": {}})
            e["names"] |= {s for s in ((p.get("flight") or "").strip(), p.get("r")) if s}
            if p.get("r"):
                e["identifiers"]["registration"] = p["r"]
            e["attributes"] |= {k: p[k] for k in ("t", "desc", "ownOp") if p.get(k)} | \
                {"military": bool((p.get("dbFlags") or 0) & 1)}
        elif x["source_id"].startswith("ais:"):
            k = f"mmsi:{p['mmsi']}"
            e = E.setdefault(k, {"id": k, "type": "vessel", "names": set(),
                                 "identifiers": {"mmsi": p["mmsi"]}, "attributes": {}})
            e["names"] |= {p["name"]} if p.get("name") else set()
        else:
            k = f"neptun:{p['track']}"
            e = E.setdefault(k, {"id": k, "type": "aircraft", "names": set(),
                                 "identifiers": {"neptun": p["track"]}, "attributes": {}})
            e["attributes"] |= {"threat_type": p.get("threat_type")}
    return [e | {"names": sorted(e["names"]), "identifiers": json.dumps(e["identifiers"]),
                 "attributes": json.dumps(e["attributes"], ensure_ascii=False)}
            for e in E.values()]
