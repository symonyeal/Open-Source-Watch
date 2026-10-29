"""Recall of the engine's violent events against UCDP Candidate events
(docs/VALIDATION.md, UCDP reference).

A UCDP event is eligible if its date is exact (date_prec = 1) and it lies within
ca. 25 km of its coded point (where_prec ≤ 2), Candidate codebook 1.5, and its
window lies inside the engine's collection period for the region. Its window at
lag L is [d, d + 1 + L) in the region's local time, d its date: L allows for
reports published on the days after. It is recalled if an engine event of a
VIOLENT kind starts in the window within DELTA km. L = 0, 1, 2 are all
reported; none is chosen. The converse share (engine violent events with an
eligible UCDP event in reach) is not precision: UCDP codes only organized
violence with at least one death, so an unmatched engine event is not an error.
"""
from datetime import UTC, datetime, time, timedelta
from zoneinfo import ZoneInfo

import numpy as np

from engine import store

VIOLENT = ("battles", "explosions_remote_violence", "violence_against_civilians")
DELTA = 25.0          # km: the fusion δ, and the codebook's precision-2 radius
LAGS = (0, 1, 2)      # days
R_EARTH = 6371.0088   # km, IUGG mean radius, as kernels/src/lib.rs


def km(lat, lon, Lat, Lon):
    """Great-circle distances (km) from one point to arrays of points."""
    p, P = np.radians(lat), np.radians(Lat)
    a = np.sin((P - p) / 2) ** 2 + np.cos(p) * np.cos(P) * np.sin(np.radians(Lon - lon) / 2) ** 2
    return 2 * R_EARTH * np.arcsin(np.sqrt(a))


def windows(U, tz, lag):
    """[start, end) in UTC of each UCDP event's window at lag days."""
    z = ZoneInfo(tz)
    s = [datetime.combine(datetime.fromisoformat(u["date_start"]).date(), time(), z)
         .astimezone(UTC) for u in U]
    return s, [x + timedelta(days=1 + lag) for x in s]


def match(U, E, tz, lag, period):
    """(eligible, recalled, engine events in reach, engine events considered) of
    UCDP events U (dicts) and engine events E (dicts t_start, lat, lon) at lag,
    within period = (first, last) collection time."""
    U = [u for u in U if u["date_prec"] == 1 and u["where_prec"] <= 2]
    s, e = windows(U, tz, lag)
    keep = [k for k in range(len(U)) if period[0] <= s[k] and e[k] <= period[1]]
    if not E:
        return len(keep), 0, 0, 0
    t = np.array([x["t_start"].timestamp() for x in E])
    lat, lon = np.array([x["lat"] for x in E]), np.array([x["lon"] for x in E])
    hit, reach = 0, np.zeros(len(E), bool)
    for k in keep:
        m = (t >= s[k].timestamp()) & (t < e[k].timestamp()) & \
            (km(U[k]["latitude"], U[k]["longitude"], lat, lon) <= DELTA)
        hit += bool(m.any())
        reach |= m
    if keep:
        lo, hi = min(s[k] for k in keep).timestamp(), max(e[k] for k in keep).timestamp()
        inside = (t >= lo) & (t < hi)
    else:
        inside = np.zeros(len(E), bool)
    return len(keep), hit, int((reach & inside).sum()), int(inside.sum())


def latest(P):
    """The latest version of each UCDP event among payloads P."""
    v = lambda p: tuple(int(x) for x in p["version"].split("."))
    out = {}
    for p in P:
        if p["id"] not in out or v(p) > v(out[p["id"]]):
            out[p["id"]] = p
    return list(out.values())


def recall(r, U):
    """Recall grid of region r against its latest UCDP events U, with the engine's
    collection period and the UCDP dates it is scored over (no grid without both)."""
    first, last = store.query(
        "SELECT min(started_at) a, max(started_at) b FROM collection_run WHERE region IN "
        "(?, 'global') AND status IN ('ok', 'empty') AND source_id NOT LIKE 'ucdp:%'",
        (r["id"],))[0].values()
    E = store.query("SELECT t_start, lat, lon FROM event WHERE region = ? AND kind IN "
                    "(SELECT unnest(?))", (r["id"], list(VIOLENT)))
    out = {"engine_period": [first and first.isoformat(), last and last.isoformat()],
           "ucdp_period": [min((u["date_start"] for u in U), default=None),
                           max((u["date_end"] for u in U), default=None)], "grid": []}
    for L in LAGS if first else ():
        n, k, a, b = match(U, E, r["timezone"], L, (first, last))
        out["grid"].append({"lag_days": L, "eligible": n, "recalled": k,
                            "recall": round(k / n, 3) if n else None,
                            "engine_events": b, "engine_in_reach": a})
    return out
