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

import numpy as np

from engine import regions, store
from engine.collect import rss, stored
from engine.process.feeds import entities

TAU, DELTA = 7200.0, 25.0          # s, km (spec § Fusion)
FINE = ("point", "city")


def components(t, lat, lon, fine, country, tau=TAU, delta=DELTA):
    """Component label of each observation, the smallest index of its component;
    t sorted ascending (seconds). Computed by the Rust kernel in kernels/
    (`py -3.14 -m pip install ./kernels`), imported here so that collection runs
    without it."""
    import osw_kernels
    return osw_kernels.components(*(np.ascontiguousarray(x, float) for x in (t, lat, lon)),
                                  *(np.ascontiguousarray(x, bool) for x in (fine, country)),
                                  tau, delta)


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


def contradicted(r, M, alerts, runs):
    """Contradiction of an air-raid-alert event M claimed by no warning source
    (spec § Fusion, amendment): some member states an event time τ
    (engine.process.when) such that the region's A/B warning source ran at least
    twice, without failure, within 10 min of τ, and recorded no alert within
    30 min of τ. A report without a stated time is never contradicted."""
    if any(x["source_id"].startswith("alerts:") for x in M):
        return False
    for x in M:
        tau = x.get("t_event")
        if tau is None:
            continue
        R = [y for y in runs if abs((y["started_at"] - tau).total_seconds()) <= 600]
        if len(R) >= 2 and all(y["status"] in ("ok", "empty") for y in R) \
                and regions.reliability(r, R[0]["source_id"]) in "AB" \
                and not any(abs((a - tau).total_seconds()) <= 1800 for a in alerts):
            return True
    return False


def events(r, X, runs=()):
    """Event rows of region r from its observations X (dicts with outlet and
    t_event) and its warning source's collection runs."""
    alerts = [x["t"] for x in X if x["source_id"].startswith("alerts:")]
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
        o = np.argsort(lab, kind="stable")              # components in index order
        for I in np.split(o, np.flatnonzero(np.diff(lab[o])) + 1):
            M = [Y[i] for i in I]
            F = I[fine[I]] if fine[I].any() else I[:1]
            S = [(regions.reliability(r, x["source_id"]), owner(r, x)) for x in M]
            out.append({
                "id": hashlib.sha256(f"evt\0{M[0]['id']}".encode()).hexdigest()[:32],
                "region": r["id"], "t_start": M[0]["t"], "t_end": M[-1]["t"],
                "lat": float(lat[F].mean()), "lon": float(lon[F].mean()), "kind": kind,
                "observation_ids": [x["id"] for x in M],
                "n_independent": len({o for g, o in S if g in "AB"}),
                "credibility": credibility(S, kind == "air_raid_alert"
                                           and contradicted(r, M, alerts, runs))})
    return out


def fuse():
    """Recompute the event and entity tables from the store (pure function of it)."""
    R, O = regions.regions(), rss.outlets()
    X = [x | {"outlet": O.get(x["raw_id"])}
         for x in store.query("SELECT * FROM observation WHERE kind <> 'report'")]
    runs = store.query("SELECT region, source_id, started_at, status FROM collection_run "
                       "WHERE source_id LIKE 'alerts:%'")
    E = []
    for rid, r in R.items():
        E += events(r, [x for x in X if x["region"] == rid],
                    [x for x in runs if x["region"] == rid])
    store.replace("event", E)
    store.replace("entity", entities(stored("adsb", "drones", "ais")))
    return len(E)
