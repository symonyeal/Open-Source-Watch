"""GNSS integrity cells per region and UTC day (docs/DECISIONS.md, GNSS).

Cell rule (gpsjam.org FAQ, docs/SOURCES.md): with g and b the aircraft that flew
through an H3 resolution-4 cell in one UTC day and reported good and bad
navigation accuracy,
    p = 100 max(0, b − 1) / (g + b);   low if p < 2, medium if 2 ≤ p ≤ 10, high if p > 10.
gpsjam subtracts one bad aircraft to limit false positives where few flew (its
formula has no max; the max changes no band).
Per-aircraft rule for the engine's own adsb.lol snapshots (engine.collect.integrity),
as gods-eye-view applies the US ADS-B Out minima of 14 CFR 91.227(c); it is not
a validated interference classifier and gpsjam does not publish its own:
    bad ⇔ GPS lost, or NACp < 8, or (NIC < 7 and not (NIC = 0 and NACp ≥ 10)).
A medium or high cell is a navigation-integrity anomaly. Jamming or spoofing is
one cause, avionics faults another, so neither is stated as the finding.
"""
from collections import defaultdict
from datetime import UTC, datetime, timedelta

import numpy as np

from engine import process
from engine.collect import stored

RES = 4
BANDS = ("low", "medium", "high")
TOWN = 100_000   # population of a city that names a cell (regions/geonames.tsv)


def pct(g, b):
    return 100 * max(0, b - 1) / (g + b) if g + b else 0.0


def band(p):
    return "high" if p > 10 else "medium" if p >= 2 else "low"


def bad(nic, nacp, lost):
    """The per-aircraft rule; None stands for a field the aircraft did not report."""
    return bool(lost or (nacp is not None and nacp < 8)
                or (nic is not None and nic < 7 and not (nic == 0 and (nacp or 0) >= 10)))


def gpsjam(r, t0):
    """{day: payload} of gpsjam.org cells stored for region r since t0."""
    return {x["payload"]["date"]: x["payload"]
            for x in stored("gpsjam", regions=[r["id"]], since=t0)}


def adsb(r, t0):
    """{day: [[cell, g, b]]} from the engine's own adsb.lol snapshots since t0:
    each aircraft counted once per cell-day, bad if any of its reports there was
    (osw_kernels.aircraft_counts)."""
    import osw_kernels
    A = [(x["published_at"], a) for x in stored("integrity", regions=[r["id"]], since=t0)
         for a in x["payload"]["ac"]]
    if not A:
        return {}
    day = np.array([t.date().toordinal() for t, _ in A], np.int64)
    lat, lon = (np.array([a[k] for _, a in A], float) for k in (1, 2))
    # A '~' address is not an ICAO address (TIS-B, non-ICAO); bit 24 keeps them apart.
    icao = np.array([int(a[0].lstrip("~"), 16) | (a[0].startswith("~") << 24) for _, a in A],
                    np.uint32)
    D, H, G, B = osw_kernels.aircraft_counts(
        day, osw_kernels.cells(lat, lon, RES), icao,
        np.array([bad(a[3], a[4], a[5]) for _, a in A]))
    out = defaultdict(list)
    for d, h, g, b in zip(D.tolist(), H.tolist(), G.tolist(), B.tolist()):
        out[datetime.fromordinal(d).date().isoformat()].append([f"{h:x}", g, b])
    return dict(out)


def summary(day, cells, source, suspect=False):
    n = defaultdict(int)
    for _, g, b in cells:
        n[band(pct(g, b))] += 1
    return {"date": day, "source": source, "suspect": suspect, "cells": len(cells),
            **{k: n[k] for k in BANDS}}


def anomalies(cells):
    """Medium and high cells with their centres, boundaries and nearest city."""
    import osw_kernels
    C = [(h, g, b, pct(g, b)) for h, g, b in cells if band(pct(g, b)) != "low"]
    if not C:
        return []
    H = np.array([int(h, 16) for h, *_ in C], np.uint64)
    lat, lon = osw_kernels.centres(H)
    return [{"h3": h, "lat": round(a, 4), "lon": round(o, 4),
             "near": process.nearest(a, o, TOWN), "good": g, "bad": b, "pct": round(p, 1),
             "band": band(p),
             "boundary": [[round(y, 4), round(x, 4)] for y, x in edge]}
            for (h, g, b, p), a, o, edge in zip(C, lat.tolist(), lon.tolist(),
                                                osw_kernels.boundaries(H))]


def agreement(J, A):
    """Cell-days present in both gpsjam (J) and the engine's snapshots (A), counted
    by whether each calls the cell an anomaly (medium or high): {"both", "gpsjam",
    "engine", "neither"}. The engine samples every CADENCE seconds where gpsjam
    sees the whole day, so this measures agreement, not either one's accuracy."""
    n = {"both": 0, "gpsjam": 0, "engine": 0, "neither": 0}
    for d in J.keys() & A.keys():
        j = {h: band(pct(g, b)) != "low" for h, g, b in J[d]["cells"]}
        for h, g, b in A[d]:
            if h in j:
                e = band(pct(g, b)) != "low"
                n["both" if j[h] and e else "gpsjam" if j[h] else "engine" if e else "neither"] += 1
    return n


def view(r, t=None, days=7):
    """Daily band counts from both sources over `days`, the latest gpsjam day's
    anomaly cells (the map), and the agreement of the two sources."""
    t = t or datetime.now(UTC)
    t0 = datetime.combine(t.date() - timedelta(days=days), datetime.min.time(), UTC)
    J, A = gpsjam(r, t0), adsb(r, t0)
    last = max(J, default=None)
    return {"days": [summary(d, J[d]["cells"], "gpsjam.org", J[d]["suspect"]) for d in sorted(J)]
            + [summary(d, A[d], "adsb.lol") for d in sorted(A)],
            "map": {"date": last, "source": "gpsjam.org",
                    "cells": anomalies(J[last]["cells"]) if last else []},
            "agreement": agreement(J, A)}


def measure(i, r, since):
    """Value of indicator i of metric "gnss" (engine.analyze): the cells at band(i)
    or above on the latest gpsjam day on or after the date `since`; date None if
    none is stored."""
    J = gpsjam(r, datetime.combine(since, datetime.min.time(), UTC))
    last = max(J, default=None)
    k = BANDS.index(i["band"])
    n = sum(BANDS.index(band(pct(g, b))) >= k for _, g, b in J[last]["cells"]) if last else 0
    return {"count": n, "date": last, "band": BANDS[k]}
