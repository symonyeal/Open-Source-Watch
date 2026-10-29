"""Signals of a region (/api/signals): GNSS integrity cells (engine.gnss), internet
outages (IODA), official daily counts (Taiwan MND) and reference events (UCDP
Candidate), each with its dates, units and collection health. Descriptive
throughout: nothing here has a threshold.
"""
import json
import statistics
from collections import Counter, defaultdict
from datetime import UTC, datetime, timedelta

from engine import gnss, store, validate

ADAPTERS = ("gpsjam", "integrity", "mnd", "ioda", "ucdp")
COUNTS = ("aircraft", "ships", "official", "balloons", "adiz", "median_line")
PRIOR = 30   # reports whose median the latest is shown beside


def official(r, n=90):
    """The region's latest n MND reports, one per reporting day (the highest
    report number wins), and each count's median over the PRIOR reports before
    the latest; a count a report does not state is left out of its median."""
    P = {}
    for x in store.query("SELECT payload FROM raw_report WHERE region = ? AND source_id "
                         "LIKE 'mnd:%'", (r["id"],)):
        p = json.loads(x["payload"])
        if p["day"] not in P or int(p["id"]) > int(P[p["day"]]["id"]):
            P[p["day"]] = p
    S = [P[d] for d in sorted(P)[-n:]]
    prior = S[-PRIOR - 1:-1]
    med = {k: statistics.median(v) if (v := [p[k] for p in prior if p[k] is not None]) else None
           for k in COUNTS}
    return {"reports": S, "latest": S[-1] if S else None, "median": med, "n_prior": len(prior),
            "unit": "per 06:00-06:00 (UTC+8) report",
            "source": "Taiwan Ministry of National Defense"}


def indicator(i, r, t, health):
    """Indicator of metric "official": count `field` of the latest MND report
    whose day is within 3 days of t, beside its median over the PRIOR reports
    before; descriptive. No data if no MND source is healthy or no report is."""
    o = official(r, PRIOR + 1)
    p = o["latest"]
    recent = p and t.date() - datetime.fromisoformat(p["day"]).date() <= timedelta(days=3)
    ok = any(h == "ok" for s, h in health.items() if s.startswith("mnd:"))
    return {"id": i["id"], "name": i["name"], "metric": "official", "field": i["field"],
            "count": (p or {}).get(i["field"]) or 0, "date": p["day"] if p else None,
            "median": o["median"][i["field"]], "n_prior": o["n_prior"],
            "status": "observed" if ok and recent else "no data", "sources": ["mnd:Taiwan MND"]}


def outages(r, t, days=7):
    """IODA events of the region's countries starting in the last `days`, one per
    (country, datasource, start) at its longest listed duration, newest first."""
    E = {}
    for x in store.query("SELECT payload, url, fetched_at FROM raw_report WHERE region = ? "
                         "AND source_id LIKE 'ioda:%' AND published_at >= ?",
                         (r["id"], t - timedelta(days=days))):
        p = json.loads(x["payload"])
        k = (p["country"], p["datasource"], p["start"])
        if k not in E or p["duration"] > E[k]["duration"]:
            E[k] = p | {"url": x["url"], "fetched_at": x["fetched_at"].isoformat()}
    C = {c["code"]: c["name"] for c in r.get("outages", {}).get("countries", [])}
    return {"countries": C, "events": [
        e | {"country_name": C.get(e["country"], e["name"]),
             "start_utc": datetime.fromtimestamp(e["start"], UTC).isoformat(),
             "hours": round(e["duration"] / 3600, 1)}
        for e in sorted(E.values(), key=lambda e: -e["start"])]}


def reference(r):
    """The region's UCDP Candidate events (latest version of each): totals by
    month, country and type of violence, and the recall grid of engine.validate."""
    U = validate.latest([json.loads(x["payload"]) for x in store.query(
        "SELECT payload FROM raw_report WHERE region = ? AND source_id LIKE 'ucdp:%'",
        (r["id"],))])
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
    mnd = any(s["id"].startswith("mnd:") for s in sources)
    return {"region": r["id"], "gnss": gnss.view(r, t), "official": official(r) if mnd else None,
            "outages": outages(r, t), "reference": reference(r),
            "sources": [s for s in sources if s["id"].split(":", 1)[0] in ADAPTERS],
            "updated": t.isoformat()}
