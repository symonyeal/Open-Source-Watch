"""Mechanical stages of the cycle: collect → process → store; the recording
proxy; rebuild.

A source id is "<adapter>:<name>[#k]"; COLLECTOR maps the adapter to its collector
module, DERIVE to its processor, FETCH to its own fetch where one URL is not one
request (a list page and its reports, a dated window). Sources sharing a URL are
fetched once per poll, and each is parsed by its own adapter. The latest response
per URL is kept in CACHE, from which the proxy answers the front end (spec § Seam):
what the dashboard shows is what was stored.
"""
import logging
import queue
import threading
import time
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from datetime import timedelta
from functools import cache
from types import SimpleNamespace
from urllib.parse import urlsplit

import httpx

from engine import analyze, fuse, produce, regions, store
from engine.collect import (
    adapter, ais, adsb, alerts, context, decode, firms, gpsjam, integrity, ioda, markets, mnd,
    neptun, now, rss, run, telegram, ucdp,
)
from engine.process import feeds, news
from engine.schema import DERIVED

# In source order: of sources sharing a URL, the first is the one the proxy always polls.
COLLECTOR = {"rss": rss, "gnews": rss, "telegram": telegram, "firms": firms, "adsb": adsb,
             "alerts": alerts, "drones": neptun, "markets": markets, "ais": ais,
             "context": context, "gpsjam": gpsjam, "integrity": integrity, "mnd": mnd,
             "ioda": ioda, "ucdp": ucdp}
FETCH = {"gpsjam": gpsjam.fetch, "mnd": mnd.fetch, "ioda": ioda.fetch, "ucdp": ucdp.fetch}
DERIVE = {"rss": news.derive, "gnews": news.derive, "telegram": feeds.telegram,
          "firms": feeds.firms, "adsb": feeds.adsb, "alerts": feeds.alerts,
          "drones": feeds.neptun, "ais": feeds.ais, "ioda": feeds.ioda}

CACHE = {}          # url -> (monotonic time, httpx.Response)
POLLED = {}         # (region, source id) -> monotonic time of its last poll in this process
SEEN = timedelta(days=8)   # longer than any source's lookback (the FIRMS 7-day file)
_seen, _lock = None, threading.Lock()
log = logging.getLogger("engine")


def sources(region_ids=None, adapters=None, polled=False):
    """Sources of the given regions and adapters; polled=True drops stream sources."""
    S = [s for m in dict.fromkeys(COLLECTOR.values()) for s in m.sources(regions.regions())]
    return [s for s in S if (not region_ids or s["region"] in region_ids)
            and (not adapters or adapter(s["id"]) in adapters)
            and not (polled and s.get("stream"))]


def due(S, t=None):
    """Sources of S of which 90 % of the cadence has elapsed since their last
    successful run in the store (a stateless runner, such as the cron, polls only
    these; the 10 % absorbs its schedule jitter)."""
    t = t or now()
    last = {(x["region"], x["source_id"]): x["t"] for x in store.query(
        "SELECT region, source_id, max(started_at) t FROM collection_run WHERE status IN "
        "('ok', 'empty') GROUP BY region, source_id")}
    return [s for s in S if (s["region"], s["id"]) not in last
            or (t - last[s["region"], s["id"]]).total_seconds() >= 0.9 * s["cadence"]]


def health(runs, cadence, t):
    """ok | stale | failed | unknown (spec § Errors); runs newest first."""
    if not runs:
        return "unknown"
    good = [x for x in runs if x["status"] in ("ok", "empty")]
    if len(runs) >= 3 and not any(x in good for x in runs[:3]):
        return "failed"
    if not good or (t - good[0]["started_at"]).total_seconds() > 3 * cadence:
        return "stale"
    return "ok"


def source_table(r):
    """Every source serving region r, with its grade, owner and health."""
    t, S = now(), sources([r["id"], "global"])
    horizon = max((3 * s["cadence"] for s in S), default=86400)
    runs = defaultdict(list)
    for x in store.query("SELECT * FROM collection_run WHERE region IN (?, 'global') "
                         "AND started_at > ? ORDER BY started_at DESC",
                         (r["id"], t - timedelta(seconds=horizon))):
        runs[x["region"], x["source_id"]].append(x)
    return [{"id": s["id"], "region": s["region"], "url": s["url"],
             "reliability": regions.reliability(r, s["id"]), "owner": regions.owner(r, s["id"]),
             "cadence_s": s["cadence"], "health": health(runs[s["region"], s["id"]],
                                                         s["cadence"], t),
             "last_status": runs[s["region"], s["id"]][0]["status"]
                 if runs[s["region"], s["id"]] else None,
             "last_run": runs[s["region"], s["id"]][0]["started_at"]
                 if runs[s["region"], s["id"]] else None}
            for s in S]


def board(r):
    """Indicator board of region r with current source health (engine.analyze)."""
    return analyze.board(r, {s["id"]: s["health"] for s in source_table(r)})


def analyse(products=()):
    """After fusion: append each region's indicator snapshot and the given products."""
    S, P = [], []
    for r in regions.regions().values():
        B = board(r)
        S += analyze.snapshot(r, B)
        P += [produce.product(r, k, produce.report(r, B, source_table(r), k)) for k in products]
    store.append("indicator_status", S)
    store.append("product", P)


@cache
def by_url():
    U = defaultdict(list)
    for s in sources():
        U[s["url"]].append(s)
    return dict(U)


def seen():
    """{raw report id: last time a poll listed it} over the last SEEN. Read from
    every stored copy, then refreshed by each poll, so an item that stays in its
    feed is stored once (a stateless runner stores it again at most once per SEEN)."""
    global _seen
    if _seen is None:
        _seen = {x["id"]: x["t"] for x in store.query(
            "SELECT id, max(fetched_at) t FROM raw_report_rows WHERE fetched_at > ? "
            "GROUP BY id", (now() - SEEN,))}
    return _seen


def prune():
    with _lock:
        S, t = seen(), now() - SEEN
        for k in [k for k, v in S.items() if v < t]:
            del S[k]


def derive(raws):
    """{table: rows} derived from raw reports, as stored, by their adapters' processors."""
    out, R = defaultdict(list), regions.regions()
    for a in {adapter(x["source_id"]) for x in raws} & DERIVE.keys():
        X = [decode(x) for x in raws if adapter(x["source_id"]) == a]
        for t, rows in DERIVE[a](X, R).items():
            out[t] += rows
    return out


def collect(S, runner="local", workers=16):
    """Poll every URL of sources S once, in parallel; store new raw reports, what
    they derive, and every run row. Returns the run rows. A URL whose processing
    raises is logged and stores nothing, so its reports stay new for the next poll
    and the other URLs are still stored."""
    G = defaultdict(list)
    for s in S:
        G[s["url"]].append(s)
        POLLED[s["region"], s["id"]] = time.monotonic()

    def one(group):
        a = adapter(group[0]["id"])
        res, raw, runs = run(group, lambda b, s: COLLECTOR[adapter(s["id"])].parse(b, s),
                             runner, **({"get": FETCH[a]} if a in FETCH else {}))
        if res is not None and a not in FETCH:   # an adapter's own fetch is not the URL's response
            CACHE[group[0]["url"]] = (time.monotonic(), res)
        try:
            return keep(raw, runs)
        except Exception:
            log.exception("processing %s failed", group[0]["url"])
            return [], {}, []

    base = [g for g in G.values() if g[0]["cadence"] >= 86400]   # baselines (FIRMS 7 d) first
    out = [one(g) for g in base]
    save(out)
    with ThreadPoolExecutor(workers) as ex:
        return [x for o in out for x in o[2]] + \
            save(list(ex.map(one, [g for g in G.values() if g not in base])))


def keep(raw, runs):
    """(new raw reports, what they derive, run rows); a listed report is not new,
    and a report is marked listed only once it has derived. A feed listing one
    item twice yields one report."""
    with _lock:
        K = seen()
        new = list({x["id"]: x for x in raw[::-1] if x["id"] not in K}.values())[::-1]
    D = derive(new)
    with _lock:
        K.update((x["id"], x["fetched_at"]) for x in raw)
    return new, D, runs


def save(out):
    store.append("raw_report", [x for o in out for x in o[0]])
    for t in DERIVED:
        store.append(t, [x for o in out for x in o[1].get(t, [])])
    runs = [x for o in out for x in o[2]]
    store.append("collection_run", runs)
    return runs


def ingest(src, messages, runner="local"):
    """Store one batch of a stream source's messages as one poll; without the
    source's key the poll is `blocked`."""
    if not ais.key():
        return save([([], {}, [{"source_id": src["id"], "region": src["region"],
                                "started_at": now(), "status": "blocked", "n_rows": 0,
                                "latency_ms": 0, "runner": runner}])])
    body = "\n".join(messages).encode()
    _, raw, runs = run([src], COLLECTOR[adapter(src["id"])].parse, runner,
                       get=lambda s: SimpleNamespace(content=body))
    return save([keep(raw, runs)])


def proxy(url):
    """The latest response for url (spec § Seam). A scheduled URL is answered from
    CACHE while fresh, else collected now, with each other source of the URL only
    if its own cadence has elapsed (an integrity snapshot shares the aircraft URL);
    an unscheduled URL on a scheduled host is fetched and recorded as source
    "unscheduled:<host>"; any other is refused."""
    S = by_url().get(url)
    if S:
        c = CACHE.get(url)
        t = time.monotonic()
        if not c or t - c[0] > max(2 * S[0]["cadence"], 30):
            collect(S[:1] + [s for s in S[1:] if t - POLLED.get((s["region"], s["id"]), -1e18)
                             >= 0.9 * s["cadence"]])
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
    every `fusion` seconds recompute events and snapshot the indicator board,
    storing a SITREP every 6 h and an INTSUM every 24 h; compact the store and
    prune the seen set every `housekeeping` seconds. Stream sources (AIS) buffer
    their messages, which are stored once per cadence. A step that raises is
    logged and retried at its next due time; the scheduler keeps running."""
    S, due, fused, kept = sources(polled=True), {}, 0.0, time.monotonic()
    made = {k: time.monotonic() for k in produce.PERIOD}
    streams = [s for s in sources() if s.get("stream")]
    Q = {s["id"]: queue.SimpleQueue() for s in streams}
    if ais.key():
        for s in streams:
            threading.Thread(target=ais.stream, args=(ais.key(), regions.regions(),
                                                      Q[s["id"]].put, stop), daemon=True).start()
    def step(f, *a):
        try:
            f(*a)
        except Exception:
            log.exception("scheduled %s failed", f.__name__)

    def products(t):
        fuse.fuse()
        P = [k for k, h in produce.PERIOD.items() if t - made[k] > 3600 * h]
        analyse(P)
        made.update({k: t for k in P})

    while not stop.is_set():
        t = time.monotonic()
        D = [s for s in S if due.get(s["id"], 0) <= t]
        due |= {s["id"]: t + s["cadence"] for s in D}
        if D:
            step(collect, D, runner)
        for s in streams:
            if due.get(s["id"], 0) <= t:
                due[s["id"]] = t + s["cadence"]
                M = []
                while not Q[s["id"]].empty():
                    M.append(Q[s["id"]].get())
                step(ingest, s, M, runner)
        if t - fused > fusion:
            fused = t
            step(products, t)
        if t - kept > housekeeping:
            kept = t
            step(store.compact)
            step(prune)
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
