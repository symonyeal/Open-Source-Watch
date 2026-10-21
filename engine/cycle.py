"""Mechanical stages of the cycle: collect → process → store; the recording
proxy; rebuild.

A source id is "<adapter>:<name>[#k]"; PARSE and DERIVE map the adapter to its
parser and processor. Sources sharing a URL are fetched once per poll. The latest
response per URL is kept in CACHE, from which the proxy answers the front end
(spec § Seam): what the dashboard shows is what was stored.
"""
import threading
import time
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from datetime import timedelta
from functools import cache
from urllib.parse import urlsplit

import httpx

from engine import fuse, regions, store
from engine.collect import adsb, alerts, firms, markets, neptun, now, rss, run, telegram
from engine.process import feeds, news
from engine.schema import DERIVED

MODULES = (rss, telegram, firms, adsb, alerts, neptun, markets)
PARSE = {"rss": rss.parse, "gnews": rss.parse, "telegram": telegram.parse, "firms": firms.parse,
         "adsb": adsb.parse, "alerts": alerts.parse, "drones": neptun.parse,
         "markets": markets.parse}
DERIVE = {"rss": news.derive, "gnews": news.derive, "telegram": feeds.telegram,
          "firms": feeds.firms, "adsb": feeds.adsb, "alerts": feeds.alerts,
          "drones": feeds.neptun}

CACHE = {}          # url -> (monotonic time, httpx.Response)
_seen, _lock = None, threading.Lock()


def adapter(source_id):
    return source_id.split(":", 1)[0]


def sources(region_ids=None, adapters=None):
    S = [s for m in MODULES for s in m.sources(regions.regions())]
    return [s for s in S if (not region_ids or s["region"] in region_ids)
            and (not adapters or adapter(s["id"]) in adapters)]


@cache
def by_url():
    U = defaultdict(list)
    for s in sources():
        U[s["url"]].append(s)
    return dict(U)


def seen():
    """{raw report id: fetched_at} of the last 2 days."""
    global _seen
    if _seen is None:
        _seen = {x["id"]: x["fetched_at"] for x in store.query(
            "SELECT id, fetched_at FROM raw_report WHERE fetched_at > ?",
            (now() - timedelta(days=2),))}
    return _seen


def prune():
    with _lock:
        S, t = seen(), now() - timedelta(days=2)
        for k in [k for k, v in S.items() if v < t]:
            del S[k]


def derive(raws):
    """{table: rows} derived from raw reports by their adapters' processors."""
    out, R = defaultdict(list), regions.regions()
    for a in {adapter(x["source_id"]) for x in raws}:
        if a in DERIVE:
            for t, rows in DERIVE[a]([x for x in raws if adapter(x["source_id"]) == a], R).items():
                out[t] += rows
    return out


def collect(S, runner="local", workers=16):
    """Poll every URL of sources S once, in parallel; store new raw reports, what
    they derive, and every run row. Returns the run rows."""
    G = defaultdict(list)
    for s in S:
        G[s["url"]].append(s)

    def one(group):
        res, raw, runs = run(group, PARSE[adapter(group[0]["id"])], runner)
        if res is not None:
            CACHE[group[0]["url"]] = (time.monotonic(), res)
        with _lock:
            K = seen()
            new = [x for x in raw if x["id"] not in K]
            K.update((x["id"], x["fetched_at"]) for x in new)
        return new, derive(new), runs

    with ThreadPoolExecutor(workers) as ex:
        out = list(ex.map(one, G.values()))
    store.append("raw_report", [x for o in out for x in o[0]])
    for t in DERIVED:
        store.append(t, [x for o in out for x in o[1].get(t, [])])
    runs = [x for o in out for x in o[2]]
    store.append("collection_run", runs)
    return runs


def proxy(url):
    """The latest response for url (spec § Seam). A scheduled URL is answered from
    CACHE while fresh, else collected now; an unscheduled URL on a scheduled host
    is fetched and recorded as source "unscheduled:<host>"; any other is refused."""
    S = by_url().get(url)
    if S:
        c = CACHE.get(url)
        if not c or time.monotonic() - c[0] > max(2 * S[0]["cadence"], 30):
            collect(S)
            c = CACHE.get(url)
        return c[1] if c else None
    host = urlsplit(url).hostname
    if host not in {urlsplit(u).hostname for u in by_url()}:
        return None
    started, t0 = now(), time.monotonic()
    try:
        res = httpx.get(url, timeout=10.0, follow_redirects=True)
        status = "ok" if res.status_code < 400 else "http_error"
    except httpx.HTTPError:
        res, status = None, "http_error"
    store.append("collection_run", [{
        "source_id": f"unscheduled:{host}", "region": "global", "started_at": started,
        "status": status, "n_rows": 0, "latency_ms": int(1000 * (time.monotonic() - t0)),
        "runner": "proxy"}])
    return res


def schedule(stop, runner="local", tick=15, fusion=600, housekeeping=3600):
    """Collect each source whenever its cadence has elapsed, until stop is set;
    recompute events every `fusion` seconds; compact the store and prune the seen set
    every `housekeeping` seconds."""
    S, due, fused, kept = sources(), {}, 0.0, time.monotonic()
    while not stop.is_set():
        t = time.monotonic()
        D = [s for s in S if due.get(s["id"], 0) <= t]
        if D:
            collect(D, runner)
            for s in D:
                due[s["id"]] = t + s["cadence"]
        if t - fused > fusion:
            fuse.fuse()
            fused = t
        if t - kept > housekeeping:
            store.compact()
            prune()
            kept = t
        stop.wait(tick)


def rebuild():
    """Regenerate every derived table from raw_report (spec § Errors)."""
    for t in DERIVED:
        store.drop(t)
    raws = store.query("SELECT * FROM raw_report ORDER BY fetched_at")
    for t, rows in derive(raws).items():
        if t not in ("event", "entity"):
            store.append(t, rows)
    fuse.fuse()
    return len(raws)
