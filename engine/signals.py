"""Signals of a region (/api/signals): GNSS integrity cells (engine.gnss), internet
outages (IODA), official daily counts (Taiwan MND) and reference events (UCDP
Candidate), each with its dates, units and collection health. Descriptive
throughout: nothing here has a threshold.
"""
import statistics
from collections import Counter, defaultdict
from datetime import UTC, datetime, timedelta

from engine import gnss, validate
from engine.collect import adapter, ioda, mnd, ucdp

ADAPTERS = ("gpsjam", "integrity", "mnd", "ioda", "ucdp")
PRIOR = 30   # reports whose median the latest is shown beside


def official(r, n=90):
    """The region's latest n MND reports, one per reporting day (mnd.reports),
    and each count's median over the PRIOR reports before the latest; a count a
    report does not state is left out of its median."""
    S = mnd.reports(r)[-n:]
    prior = S[-PRIOR - 1:-1]
    med = {k: statistics.median(v) if (v := [p[k] for p in prior if p[k] is not None]) else None
           for k in mnd.FIELDS}
    return {"reports": S, "latest": S[-1] if S else None, "median": med, "n_prior": len(prior),
            "unit": "per 06:00-06:00 (UTC+8) report",
            "source": "Taiwan Ministry of National Defense"}


def measure(i, r, since):
    """Value of indicator i of metric "official" (engine.analyze): count field(i)
    of the latest MND report if its day is on or after the date `since` (date
    None otherwise), beside the count's median over the PRIOR reports before it."""
    o = official(r, PRIOR + 1)
    p = o["latest"] if o["latest"] and o["latest"]["day"] >= since.isoformat() else None
    return {"field": i["field"], "count": (p or {}).get(i["field"]) or 0,
            "date": p and p["day"], "median": o["median"][i["field"]], "n_prior": o["n_prior"]}


def outages(r, t, days=7):
    """IODA events of the region's countries starting in the last `days`, each at
    its longest listed duration (ioda.events), newest first."""
    C = {c["code"]: c["name"] for c in r.get("outages", {}).get("countries", [])}
    return {"countries": C, "events": [
        e | {"fetched_at": e["fetched_at"].isoformat(),
             "country_name": C.get(e["country"], e["name"]),
             "start_utc": datetime.fromtimestamp(e["start"], UTC).isoformat(),
             "hours": round(e["duration"] / 3600, 1)}
        for e in sorted(ioda.events(r, t - timedelta(days=days)), key=lambda e: -e["start"])]}


def reference(r):
    """The region's UCDP Candidate events (latest version of each): totals by
    month, country and type of violence, and the recall grid of engine.validate."""
    U = ucdp.events(r)
    if not U:
        return None
    by = defaultdict(lambda: {"events": 0, "deaths_best": 0})
    for u in U:
        for k in (("month", u["date_start"][:7]), ("country", u["country"])):
            by[k]["events"] += 1
            by[k]["deaths_best"] += u["best"]
    group = lambda g: {k: v for (h, k), v in sorted(by.items()) if h == g}
    return {"versions": sorted({u["version"] for u in U}), "events": len(U),
            "first": min(u["date_start"] for u in U), "last": max(u["date_end"] for u in U),
            "by_month": group("month"), "by_country": group("country"),
            "by_type": dict(Counter({1: "state-based", 2: "non-state", 3: "one-sided"}
                                    [u["type_of_violence"]] for u in U)),
            "eligible_share": round(sum(u["date_prec"] == 1 and u["where_prec"] <= 2
                                        for u in U) / len(U), 3),
            "recall": validate.recall(r, U)}


def view(r, sources, t=None):
    """The signals of region r; `sources` is its source table (cycle.source_table)."""
    t = t or datetime.now(UTC)
    claims = any(adapter(s["id"]) == "mnd" for s in sources)
    return {"region": r["id"], "gnss": gnss.view(r, t), "official": official(r) if claims else None,
            "outages": outages(r, t), "reference": reference(r),
            "sources": [s for s in sources if adapter(s["id"]) in ADAPTERS],
            "updated": t.isoformat()}
