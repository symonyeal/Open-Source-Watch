"""Analysis, stage 5: indicators, transits, PIR status, competing hypotheses.

Event indicator i: the events of kind(i) with credibility ≤ c(i) whose t_end
lies in the last window(i) hours are counted;
    count ≥ trigger(i) → triggered;  count ≥ watch(i) → watch;  else inactive.
Transit indicator i (metric "transit"): crossings of gate(i) per UTC day by AIS
tracks; the last complete day's count n is compared with the window(i) days
before it, z = (n − mean) / sd;
    z ≤ trigger_z → triggered;  z ≤ watch_z → watch;  else inactive;
fewer than 7 prior days with AIS coverage → no data.
A PIR is answered if one indicator is triggered; a collection gap if, for some
indicator, no healthy source can produce its evidence (PRODUCERS); otherwise
open. A kind that did not occur is not a gap.
ACH (Heuer 1999, ch. 8): evidence = indicators, present iff watch or triggered,
absent iff inactive; a hypothesis's score is the number of items whose state
contradicts its expectation; hypotheses are ranked by fewest inconsistencies.
"""
from collections import defaultdict
from datetime import UTC, datetime, timedelta
from itertools import pairwise

import numpy as np

from engine import store

MIN_DAYS = 7
TEXT = ("rss", "gnews", "telegram")
# Adapters whose sources can produce each kind; the rest come from text sources.
PRODUCERS = {"thermal_anomaly": ("firms",), "airspace_activity": ("drones",) + TEXT,
             "air_raid_alert": ("alerts",) + TEXT}


def gap(kind, health):
    """No healthy source of the region can produce `kind` (spec § Errors)."""
    A = PRODUCERS.get(kind, TEXT)
    return not any(h == "ok" for s, h in health.items() if s.split(":", 1)[0] in A)


def status(n, watch, trigger):
    return "triggered" if n >= trigger else "watch" if n >= watch else "inactive"


def orient(u, v, w):
    """Sign of the turn u → v → w (cross product)."""
    return np.sign((v[0] - u[0]) * (w[1] - u[1]) - (v[1] - u[1]) * (w[0] - u[0]))


def crosses(p, q, a, b):
    """Segment pq properly crosses segment ab (planar lat/lon, short segments)."""
    return orient(p, q, a) * orient(p, q, b) < 0 and orient(a, b, p) * orient(a, b, q) < 0


def crossings(P, line):
    """Crossing times of `line` by one vessel's track points P (sorted by t)."""
    a, b = line
    return [y["t"] for x, y in pairwise(P)
            if crosses((x["lat"], x["lon"]), (y["lat"], y["lon"]), a, b)]


def daily_transits(r, gate, t, days):
    """({day: crossings}, {days with AIS coverage}) over the days + 1 complete UTC
    days before t's day."""
    t0 = datetime.combine(t.date() - timedelta(days=days + 1), datetime.min.time(), UTC)
    T = store.query("SELECT entity_id, t, lat, lon FROM track_point WHERE region = ? AND "
                    "entity_id LIKE 'mmsi:%' AND t >= ? AND t < ? ORDER BY entity_id, t",
                    (r["id"], t0, datetime.combine(t.date(), datetime.min.time(), UTC)))
    V, C = defaultdict(list), defaultdict(int)
    for x in T:
        V[x["entity_id"]].append(x)
    for P in V.values():
        for c in crossings(P, r["gate"][gate]["line"]):
            C[c.date()] += 1
    return C, {x["t"].date() for x in T}


def transit_indicator(r, i, t):
    C, covered = daily_transits(r, i["gate"], t, i["window_days"])
    last = t.date() - timedelta(days=1)
    prior = [last - timedelta(days=k) for k in range(1, i["window_days"] + 1)]
    x = np.array([C[d] for d in prior if d in covered], float)
    out = {"id": i["id"], "name": i["name"], "metric": "transit", "count": C[last],
           "window_days": i["window_days"], "watch_z": i["watch_z"],
           "trigger_z": i["trigger_z"], "sources": ["ais:aisstream"]}
    if len(x) < MIN_DAYS or last not in covered:
        return out | {"z": None, "status": "no data"}
    sd = x.std(ddof=1)
    z = 0.0 if sd == 0 and C[last] == x.mean() else (C[last] - x.mean()) / sd if sd else \
        float(np.sign(C[last] - x.mean()) * np.inf)
    s = "triggered" if z <= i["trigger_z"] else "watch" if z <= i["watch_z"] else "inactive"
    return out | {"z": round(float(z), 2), "status": s}


def ach(p, I):
    """Hypotheses of PIR p ranked by inconsistency with indicator states I."""
    state = {i["id"]: "present" if i["status"] in ("watch", "triggered") else
             "absent" if i["status"] == "inactive" else None for i in I}
    H = []
    for h in p.get("hypothesis", []):
        E = {k: state.get(k) for k in h["expect"]}
        bad = [k for k, v in h["expect"].items() if E[k] is not None and E[k] != v]
        H.append({"id": h["id"], "text": h["text"], "inconsistent": bad,
                  "unassessed": [k for k in h["expect"] if E[k] is None]})
    return sorted(H, key=lambda h: (len(h["inconsistent"]), h["id"]))


def board(r, health, t=None):
    """[{pir, question, status, indicators, hypotheses}] of region r; health maps
    source id → ok | stale | failed | unknown."""
    t = t or datetime.now(UTC)
    E = store.query("SELECT kind, credibility, t_end FROM event WHERE region = ? AND t_end > ?",
                    (r["id"], t - timedelta(days=7)))
    P = store.query("SELECT DISTINCT kind, source_id FROM observation WHERE region = ? "
                    "AND t > ?", (r["id"], t - timedelta(days=7)))
    out = []
    for p in r["pir"]:
        I, miss = [], False
        for i in p["indicator"]:
            if i.get("metric") == "transit":
                x = transit_indicator(r, i, t)
                miss |= health.get("ais:aisstream") != "ok"
            else:
                w = t - timedelta(hours=i["window_h"])
                n = sum(1 for e in E if e["kind"] == i["kind"]
                        and e["credibility"] <= i["max_credibility"] and e["t_end"] > w)
                S = {x["source_id"] for x in P if x["kind"] == i["kind"]}
                miss |= gap(i["kind"], health)
                x = {"id": i["id"], "name": i["name"], "kind": i["kind"], "count": n,
                     "window_h": i["window_h"], "watch": i["watch"], "trigger": i["trigger"],
                     "status": status(n, i["watch"], i["trigger"]), "sources": sorted(S)}
            I.append(x)
        s = "answered" if any(x["status"] == "triggered" for x in I) else \
            "gap" if miss else "open"
        out.append({"pir": p["id"], "question": p["question"], "status": s, "indicators": I})
    I = [i for x in out for i in x["indicators"]]          # hypotheses may cite any PIR
    return [x | {"hypotheses": ach(p, I)} for x, p in zip(out, r["pir"])]


def snapshot(r, B, t=None):
    """indicator_status rows of board B at time t (the indicator board's history)."""
    t = t or datetime.now(UTC)
    return [{"region": r["id"], "t": t, "pir": p["pir"], "indicator": i["id"],
             "status": i["status"], "count": int(i["count"])}
            for p in B for i in p["indicators"]]
