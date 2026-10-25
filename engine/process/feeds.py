"""Processors of the non-news feeds: raw reports → observations and track points
(spec § Processing). One function per adapter, `derive(raws, R) -> {table: rows}`."""
import json
from collections import defaultdict
from datetime import date, timedelta
from functools import cache
from pathlib import Path

from engine import regions, store
from engine.collect.firms import key as firms_key
from engine.process import credibility_single, lang, obs_id, observations, translate

CITY = Path(__file__).resolve().parents[2] / "src" / "data" / "city-data.json"


def telegram(raws, R):
    return {"observation": observations(
        [(R[x["region"]], x, json.loads(x["payload"])["text"]) for x in raws])}


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
    H = store.query("SELECT payload FROM raw_report WHERE source_id LIKE 'firms:%' "
                    "AND published_at >= ?", (t0,))
    D = defaultdict(set)
    for p in [json.loads(h["payload"]) for h in H] + [json.loads(x["payload"]) for x in raws]:
        D[cell(float(p["latitude"]), float(p["longitude"]))].add(date.fromisoformat(p["acq_date"]))
    out = []
    for x in raws:
        p = json.loads(x["payload"])
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
        p = json.loads(x["payload"])
        out += [{"entity_id": f"mmsi:{p['mmsi']}", "region": r["id"], "t": x["published_at"],
                 "lat": p["lat"], "lon": p["lon"], "heading": p.get("cog"), "speed": p.get("sog"),
                 "source_id": x["source_id"]}
                for r in R.values() if regions.in_bbox(r, p["lat"], p["lon"])]
    return {"track_point": out}


def adsb(raws, R):
    """Track points of aircraft, entity "icao:<hex>", in each region containing them."""
    out = []
    for x in raws:
        p = json.loads(x["payload"])
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
        r, p = R[x["region"]], json.loads(x["payload"])
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


def neptun(raws, R):
    """Track points of each drone/missile track, and one airspace_activity
    observation per track (the observation id depends on the track id only)."""
    T, O = [], []
    for x in raws:
        r, p = R[x["region"]], json.loads(x["payload"])
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
