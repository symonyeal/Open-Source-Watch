"""Processors of the non-news feeds: raw reports → observations and track points
(spec § Processing). One function per adapter, `derive(raws, R) -> {table: rows}`."""
import json
from functools import cache
from pathlib import Path

from engine import regions
from engine.process import credibility_single, lang, obs_id, observation, translate

CITY = Path(__file__).resolve().parents[2] / "src" / "data" / "city-data.json"


def telegram(raws, R):
    return {"observation": [observation(R[x["region"]], x, json.loads(x["payload"])["text"])
                            for x in raws]}


def firms(raws, R):
    """One thermal_anomaly per detection per region containing it; VIIRS
    low-confidence ('l') detections are not observations."""
    out = []
    for x in raws:
        p = json.loads(x["payload"])
        lat, lon = float(p["latitude"]), float(p["longitude"])
        if p["confidence"] == "l":
            continue
        for k, r in enumerate(R.values()):
            if regions.in_bbox(r, lat, lon):
                out.append({
                    "id": obs_id(x["id"], k), "raw_id": x["id"], "source_id": x["source_id"],
                    "region": r["id"], "t": x["published_at"], "lat": lat, "lon": lon,
                    "geo_precision": "point", "kind": "thermal_anomaly",
                    "text_en": f"VIIRS thermal anomaly, FRP {p['frp']} MW, confidence "
                               f"{p['confidence']}, {p['daynight']}",
                    "lang": "en", "credibility": credibility_single(r, x["source_id"]),
                    "names": []})
    return {"observation": out}


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
                    "credibility": credibility_single(r, x["source_id"]), "names": places})
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
                  "names": [p["place"]] if p.get("place") else []})
    return {"track_point": T, "observation": O}
