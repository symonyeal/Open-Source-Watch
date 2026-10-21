"""Exploitation and fusion, stage 4 (spec § Fusion, § Credibility).

Observations o, o' of a region are linked when
    kind(o) = kind(o') ≠ report,   |t − t'| ≤ τ,   and
    d(o, o') ≤ δ  if both are located to city precision or finer,
    lat/lon equal (the same administrative unit) if neither is country-level.
Events are the connected components of this graph (single linkage), so the
partition does not depend on input order. A country-level observation links to
nothing (a single-observation event); an unlocated one forms no event.
Cost per region and kind: O(n w) distances, w = observations within τ of one.

Credibility of an event (Admiralty 1-6), first match wins, with n the number of
distinct owners among its A/B-graded sources:
    contradicted → 5;  n ≥ 3 → 1;  n = 2 → 2;  n = 1 → 3;
    any C/D/E source → 4;  otherwise → 6.
"""
import hashlib
import json

import numpy as np

from engine import regions, store

TAU, DELTA = 7200.0, 25.0          # s, km (spec § Fusion)
R_EARTH = 6371.0088                # km, IUGG mean radius
FINE = ("point", "city")


def haversine(lat1, lon1, lat2, lon2):
    """Great-circle distance in km (vectorised)."""
    p1, p2 = np.radians(lat1), np.radians(lat2)
    a = np.sin((p2 - p1) / 2) ** 2 + \
        np.cos(p1) * np.cos(p2) * np.sin(np.radians(lon2 - lon1) / 2) ** 2
    return 2 * R_EARTH * np.arcsin(np.sqrt(a))


def components(t, lat, lon, fine, country, tau=TAU, delta=DELTA):
    """Component label of each observation; t sorted ascending (seconds)."""
    n = len(t)
    parent = np.arange(n)

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    hi = np.searchsorted(t, t + tau, side="right")
    for i in range(n):
        J = np.arange(i + 1, hi[i])
        if not len(J):
            continue
        near = haversine(lat[i], lon[i], lat[J], lon[J]) <= delta
        same = (lat[J] == lat[i]) & (lon[J] == lon[i]) & ~country[i] & ~country[J]
        for j in J[np.where(fine[i] & fine[J], near, same)]:
            a, b = find(i), find(j)
            if a != b:
                parent[max(a, b)] = min(a, b)
    return np.array([find(i) for i in range(n)])


def credibility(support, contradicted=False):
    """support: [(grade, owner)] of the event's observations."""
    if contradicted:
        return 5
    n = len({o for g, o in support if g in "AB"})
    if n:
        return max(1, 4 - n)
    return 4 if any(g in "CDE" for g, _ in support) else 6


def owner(r, x):
    """Owner of an observation's source; a Google News item belongs to its outlet."""
    if x["source_id"].startswith(("gnews:", "rss:Google News")) and x.get("outlet"):
        return x["outlet"]
    return regions.owner(r, x["source_id"])


def events(r, X):
    """Event rows of region r from its observations X (dicts with outlet). No
    contradiction rule applies before P3 (docs/DECISIONS.md), so none reaches 5."""
    out = []
    for kind in sorted({x["kind"] for x in X} - {"report"}):
        Y = sorted((x for x in X if x["kind"] == kind and x["lat"] is not None),
                   key=lambda x: (x["t"], x["id"]))
        if not Y:
            continue
        t = np.array([x["t"].timestamp() for x in Y])
        lat, lon = np.array([x["lat"] for x in Y]), np.array([x["lon"] for x in Y])
        fine = np.array([x["geo_precision"] in FINE for x in Y])
        country = np.array([x["geo_precision"] == "country" for x in Y])
        lab = components(t, lat, lon, fine, country)
        for c in np.unique(lab):
            I = np.flatnonzero(lab == c)
            M = [Y[i] for i in I]
            F = I[fine[I]] if fine[I].any() else I[:1]
            S =[(regions.reliability(r, x["source_id"]), owner(r, x)) for x in M]
            out.append({
                "id": hashlib.sha256(f"evt\0{M[0]['id']}".encode()).hexdigest()[:32],
                "region": r["id"], "t_start": M[0]["t"], "t_end": M[-1]["t"],
                "lat": float(lat[F].mean()), "lon": float(lon[F].mean()), "kind": kind,
                "observation_ids": [x["id"] for x in M],
                "n_independent": len({o for g, o in S if g in "AB"}),
                "credibility": credibility(S)})
    return out


def entities(raws):
    """Aircraft entities from ADS-B and Neptun raw reports, latest attributes win."""
    E = {}
    for x in sorted(raws, key=lambda x: x["fetched_at"]):
        p = json.loads(x["payload"])
        if x["source_id"].startswith("adsb:"):
            h = p["hex"].strip().replace("~", "")
            e = E.setdefault(f"icao:{h}", {"id": f"icao:{h}", "type": "aircraft", "names": set(),
                                           "identifiers": {"icao": h}, "attributes": {}})
            e["names"] |= {s for s in ((p.get("flight") or "").strip(), p.get("r")) if s}
            if p.get("r"):
                e["identifiers"]["registration"] = p["r"]
            e["attributes"] |= {k: p[k] for k in ("t", "desc", "ownOp") if p.get(k)} | \
                {"military": bool((p.get("dbFlags") or 0) & 1)}
        else:
            k = f"neptun:{p['track']}"
            e = E.setdefault(k, {"id": k, "type": "aircraft", "names": set(),
                                 "identifiers": {"neptun": p["track"]}, "attributes": {}})
            e["attributes"] |= {"threat_type": p.get("threat_type")}
    return [e | {"names": sorted(e["names"]), "identifiers": json.dumps(e["identifiers"]),
                 "attributes": json.dumps(e["attributes"], ensure_ascii=False)}
            for e in E.values()]


def fuse():
    """Recompute the event and entity tables from the store (pure function of it)."""
    R = regions.regions()
    X = store.query(
        "SELECT o.*, json_extract_string(r.payload, '$.outlet') AS outlet FROM observation o "
        "JOIN raw_report r ON r.id = o.raw_id WHERE o.kind <> 'report'")
    E = []
    for rid, r in R.items():
        E += events(r, [x for x in X if x["region"] == rid])
    store.replace("event", E)
    T = store.query("SELECT * FROM raw_report WHERE source_id LIKE 'adsb:%' "
                    "OR source_id LIKE 'drones:%'")
    store.replace("entity", entities(T))
    return len(E)
