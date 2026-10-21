"""Analysis, stage 5: indicator and PIR status (spec § Data model).

An indicator i counts the events of kind(i) with credibility ≤ c(i) whose
t_end lies in the last window(i) hours:
    count ≥ trigger(i) → triggered;  count ≥ watch(i) → watch;  else inactive.
A PIR is answered if one of its indicators is triggered. It is a collection gap
if, for some indicator, no source that produced that kind in the last 7 days is
healthy now (spec § Errors). Otherwise it is open.
"""
from datetime import UTC, datetime, timedelta

from engine import store


def status(n, watch, trigger):
    return "triggered" if n >= trigger else "watch" if n >= watch else "inactive"


def board(r, health, t=None):
    """[{pir, question, status, indicators: [...]}] of region r; health maps
    source id → ok | stale | failed | unknown."""
    t = t or datetime.now(UTC)
    E = store.query("SELECT kind, credibility, t_end FROM event WHERE region = ? AND t_end > ?",
                    (r["id"], t - timedelta(days=7)))
    P = store.query("SELECT DISTINCT kind, source_id FROM observation WHERE region = ? "
                    "AND t > ?", (r["id"], t - timedelta(days=7)))
    out = []
    for p in r["pir"]:
        I, gap = [], False
        for i in p["indicator"]:
            w = t - timedelta(hours=i["window_h"])
            n = sum(1 for e in E if e["kind"] == i["kind"]
                    and e["credibility"] <= i["max_credibility"] and e["t_end"] > w)
            S = {x["source_id"] for x in P if x["kind"] == i["kind"]}
            gap |= not any(health.get(s) == "ok" for s in S)
            I.append({"id": i["id"], "name": i["name"], "kind": i["kind"], "count": n,
                      "window_h": i["window_h"], "watch": i["watch"], "trigger": i["trigger"],
                      "status": status(n, i["watch"], i["trigger"]), "sources": sorted(S)})
        s = "answered" if any(x["status"] == "triggered" for x in I) else \
            "gap" if gap else "open"
        out.append({"pir": p["id"], "question": p["question"], "status": s, "indicators": I})
    return out
