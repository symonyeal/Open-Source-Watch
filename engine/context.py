"""Dated energy/trade context, separate from incident-based PIR judgements.

Shipping follows the World Bank chokepoints notebook's centered seven-day mean,
restricted to complete calendar windows. The center date lags the last sample
by three days. Missing observations never become zeros.
"""
import json
from datetime import UTC, date, datetime, timedelta

from engine import cycle, store


def mean7(points, field="value"):
    if not points:
        return None
    end = date.fromisoformat(points[-1]["date"])
    values = {p["date"]: p.get(field) for p in points}
    x = [values.get((end - timedelta(days=k)).isoformat()) for k in range(7)]
    if any(v is None for v in x):
        return None
    return {"mean": round(sum(x) / 7, 2), "n": 7,
            "center_date": (end - timedelta(days=3)).isoformat(),
            "window_start": (end - timedelta(days=6)).isoformat(), "window_end": end.isoformat()}


def series(payload, fetched, health, t):
    out = {k: v for k, v in payload.items() if k != "points"}
    out.update(fetched_at=fetched.isoformat(), health=health)
    if payload["kind"] == "wdi":
        latest = {}
        for p in payload["points"]:
            if (p["value"] is not None and int(p["date"]) <= t.year
                    and (p["country"] not in latest or p["date"] > latest[p["country"]]["date"])):
                latest[p["country"]] = p
        return out | {"countries": latest}
    points = sorted((p for p in payload["points"] if p["date"] < t.date().isoformat()),
                    key=lambda p: p["date"])
    valid = [p for p in points if p["value"] is not None]
    latest = valid[-1] if valid else None
    age = (t.date() - date.fromisoformat(latest["date"])).days if latest else None
    out.update(latest=latest, age_days=age, points=points)
    if payload["kind"] == "fred":
        prev = valid[-2]["value"] if len(valid) > 1 else None
        out["change_pct"] = round(100 * (latest["value"] / prev - 1), 2) if prev else None
    else:
        out.update(mean7=mean7(points), tanker_mean7=mean7(points, "tanker"))
    return out


def view(r, t=None):
    t = t or datetime.now(UTC)
    sources = [s for s in cycle.source_table(r) if s["id"].startswith("context:")]
    health = {(s["region"], s["id"]): s["health"] for s in sources}
    rows = store.query("SELECT * FROM raw_report WHERE region IN (?, 'global') "
                       "AND source_id LIKE 'context:%' QUALIFY row_number() OVER "
                       "(PARTITION BY region, source_id ORDER BY fetched_at DESC) = 1", (r["id"],))
    data = [series(json.loads(x["payload"]), x["fetched_at"],
                   health.get((x["region"], x["source_id"]), "unknown"), t) for x in rows]
    prices = {x["code"]: x for x in data if x["kind"] == "fred"}
    brent, wti = prices.get("DCOILBRENTEU"), prices.get("DCOILWTICO")
    spread = None
    if brent and wti:
        B = {p["date"]: p["value"] for p in brent["points"] if p["value"] is not None}
        W = {p["date"]: p["value"] for p in wti["points"] if p["value"] is not None}
        dates = B.keys() & W.keys()
        if dates:
            d = max(dates)
            spread = {"date": d, "value": round(B[d] - W[d], 2), "unit": "USD/bbl"}
    return {"region": r["id"], "countries": r.get("context", {}).get("countries", []),
            "series": sorted(data, key=lambda x: (x["kind"], x["name"])), "spread": spread,
            "sources": sources, "updated": t.isoformat()}
