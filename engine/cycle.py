"""Mechanical stages of the cycle: collect → process → store, and rebuild.

ADAPTERS maps an adapter name (the prefix of a source id) to its collection
module and its processor. Raw reports already stored are not written again, so
a poll that sees nothing new writes only its collection_run row.
"""
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from datetime import timedelta

from engine import regions, store
from engine.collect import now, rss, run
from engine.process import news
from engine.schema import DERIVED

ADAPTERS = {"rss": (rss, news.observations)}

_seen, _lock = None, threading.Lock()


def seen():
    """Raw report ids stored in the last 3 days."""
    global _seen
    if _seen is None:
        since = now() - timedelta(days=3)
        _seen = {x["id"] for x in store.query(
            "SELECT id FROM raw_report WHERE fetched_at > ?", (since,))}
    return _seen


def tasks(region_ids=None, adapters=None):
    """[(region, source_id, cadence_s, fetch, adapter)] over the registry."""
    T = []
    for rid, r in regions.regions().items():
        if region_ids and rid not in region_ids:
            continue
        for a, (mod, _) in ADAPTERS.items():
            if adapters and a not in adapters:
                continue
            T += [(r, sid, c, f, a) for sid, c, f in mod.sources(r)]
    return T


def process(r, adapter, raws):
    return ADAPTERS[adapter][1](r, raws) if raws else []


def collect(T, runner="local", workers=16):
    """Poll every task once, in parallel; store new raw reports, their
    observations and every run row. Returns the run rows."""
    def one(t):
        r, sid, _, f, a = t
        raw, row = run(r["id"], sid, f, runner)
        with _lock:
            S = seen()
            new = [x for x in raw if x["id"] not in S]
            S.update(x["id"] for x in new)
        return new, process(r, a, new), row

    with ThreadPoolExecutor(workers) as ex:
        out = list(ex.map(one, T))
    store.append("raw_report", [x for o in out for x in o[0]])
    store.append("observation", [x for o in out for x in o[1]])
    store.append("collection_run", [o[2] for o in out])
    return [o[2] for o in out]


def schedule(stop, runner="local", tick=15):
    """Collect each source whenever its cadence has elapsed, until stop is set."""
    T, due = tasks(), {}
    while not stop.is_set():
        t = time.monotonic()
        D = [x for x in T if due.get(x[1], 0) <= t]
        if D:
            collect(D, runner)
            for x in D:
                due[x[1]] = t + x[2]
        stop.wait(tick)


def rebuild():
    """Regenerate every derived table from raw_report (spec § Errors)."""
    for t in DERIVED:
        store.drop(t)
    R = regions.regions()
    raws = store.query("SELECT * FROM raw_report ORDER BY fetched_at")
    for rid, r in R.items():
        for a in ADAPTERS:
            X = [x for x in raws if x["region"] == rid and x["source_id"].startswith(a + ":")]
            store.append("observation", process(r, a, X))
    return len(raws)
