"""Engine HTTP API (spec § Seam).

/fetch?url=U   the recording proxy: the latest stored response for an upstream URL
               (src/lib/fetcher.ts forwards here when ENGINE_URL is set)
/api/telegram  upstream's TelegramData shape (src/lib/types/api.ts), from the store
/api/indicators PIR and indicator status of a region
/api/context, /api/signals   recorded energy/trade context; GNSS, outages, MND, UCDP
/regions, /observations, /events, /tracks, /sources, /stream   engine-native views
"""
import asyncio
import json
import threading
from collections import defaultdict
from contextlib import asynccontextmanager
from datetime import UTC, datetime, timedelta

from starlette.applications import Starlette
from starlette.concurrency import run_in_threadpool
from starlette.exceptions import HTTPException
from starlette.responses import JSONResponse, Response, StreamingResponse
from starlette.routing import Route

from engine import cycle, regions, store

NO_CACHE = {"Cache-Control": "no-cache, no-store, must-revalidate"}
STREAM_LAG = timedelta(minutes=15)   # longer than a poll takes from fetch to write


def region_of(req):
    rid = req.query_params.get("region") or req.query_params.get("conflict")
    R = regions.regions()
    if rid and rid not in R:
        raise HTTPException(400, "Unknown region")
    return R[rid or "iran-israel"]


def since(req, hours=48):
    s = req.query_params.get("since")
    if not s:
        return datetime.now(UTC) - timedelta(hours=hours)
    try:
        t = datetime.fromisoformat(s)
    except ValueError:
        raise HTTPException(400, "since must be an ISO 8601 timestamp") from None
    if t.tzinfo is None:
        raise HTTPException(400, "since must include a timezone")
    return t.astimezone(UTC)


def limit(req):
    try:
        n = int(req.query_params.get("limit", 500))
    except ValueError:
        raise HTTPException(400, "limit must be an integer from 1 to 10000") from None
    if not 1 <= n <= 10000:
        raise HTTPException(400, "limit must be an integer from 1 to 10000")
    return n


def js(x):
    return JSONResponse(json.loads(json.dumps(x, default=str)), headers=NO_CACHE)


async def fetch(req):
    res = await run_in_threadpool(cycle.proxy, req.query_params.get("url", ""))
    if res is None:
        return Response(status_code=502)
    return Response(res.content, status_code=res.status_code, headers=NO_CACHE,
                    media_type=res.headers.get("content-type", "application/octet-stream"))


async def regions_(req):
    return js([{"id": r["id"], "name": r["name"], "bbox": r["bbox"],
                "pirs": [{"id": p["id"], "question": p["question"]} for p in r["pir"]]}
               for r in regions.regions().values()])


async def observations(req):
    r, q = region_of(req), req.query_params
    sql = ("SELECT * FROM observation WHERE region = ? AND t > ?"
           + (" AND kind = ?" if q.get("kind") else "")
           + (" AND source_id LIKE ?" if q.get("source") else "")
           + " ORDER BY t DESC LIMIT ?")
    p = [r["id"], since(req)] + ([q["kind"]] if q.get("kind") else []) \
        + ([q["source"] + "%"] if q.get("source") else []) + [limit(req)]
    return js(await run_in_threadpool(store.query, sql, p))


async def events(req):
    r, q = region_of(req), req.query_params
    sql = ("SELECT * FROM event WHERE region = ? AND t_end > ?"
           + (" AND kind = ?" if q.get("kind") else "") + " ORDER BY t_end DESC LIMIT ?")
    p = [r["id"], since(req)] + ([q["kind"]] if q.get("kind") else []) + [limit(req)]
    return js(await run_in_threadpool(store.query, sql, p))


async def tracks(req):
    r = region_of(req)
    return js(await run_in_threadpool(
        store.query, "SELECT * FROM track_point WHERE region = ? AND t > ? ORDER BY entity_id, t",
        (r["id"], since(req, 6))))


async def indicators(req):
    return js(await run_in_threadpool(cycle.board, region_of(req)))


async def products(req):
    """Latest scheduled INTSUM (default) or SITREP of the region."""
    kind = "sitrep" if req.query_params.get("kind") == "sitrep" else "intsum"
    rows = await run_in_threadpool(
        store.query,
        "SELECT * FROM product WHERE region = ? AND kind = ? ORDER BY t DESC LIMIT 1",
        (region_of(req)["id"], kind))
    return js(rows[0] if rows else None)


async def sources(req):
    return js(await run_in_threadpool(cycle.source_table, region_of(req)))


async def context(req):
    from engine.context import view
    return js(await run_in_threadpool(view, region_of(req)))


async def signals(req):
    from engine.signals import view
    r = region_of(req)
    return js(await run_in_threadpool(lambda: view(r, cycle.source_table(r))))


def telegram_data(r, t):
    """Upstream TelegramData: the latest 3 stored posts per channel, newest first
    (src/app/api/telegram/route.ts); text is the stored excerpt, translated."""
    X = store.query(
        "SELECT o.text_en, r.payload FROM observation o JOIN raw_report r ON r.id = o.raw_id "
        "WHERE o.region = ? AND o.source_id LIKE 'telegram:%' AND o.t > ?", (r["id"], t))
    C = {c["name"]: c for c in r["conflict"]["telegramChannels"]}
    by = defaultdict(list)
    for x in X:
        p = json.loads(x["payload"])
        by[p["channel"]].append((p, x["text_en"]))
    posts = []
    for ch, P in by.items():
        c = C.get(ch) or next((v for k, v in C.items() if k.lower() == ch.lower()), None)
        if c is None:
            continue
        for p, en in sorted(P, key=lambda y: -y[0]["postId"])[:3]:
            posts.append({"channel": c["name"], "channelLabel": c["label"], "color": c["color"],
                          "postId": p["postId"], "text": en, "date": p["date"],
                          "url": f"https://t.me/{c['name']}/{p['postId']}"})
    posts.sort(key=lambda p: p["date"] or "", reverse=True)
    return {"posts": posts, "channels": [c["label"] for c in C.values()],
            "updated": datetime.now(UTC).isoformat()}


async def telegram(req):
    return js(await run_in_threadpool(telegram_data, region_of(req), since(req, 24 * 7)))


async def stream(req):
    """Server-sent events: each new observation of the region, polled every 5 s.
    A report's fetched_at is its poll's start, earlier than its write, so the
    query looks back STREAM_LAG and skips observations already sent."""
    r = region_of(req)

    async def events():
        t, sent = datetime.now(UTC), {}
        while not await req.is_disconnected():
            X = await run_in_threadpool(
                store.query, "SELECT o.*, r.fetched_at FROM observation o JOIN raw_report r "
                "ON r.id = o.raw_id WHERE o.region = ? AND r.fetched_at > ? ORDER BY o.t",
                (r["id"], max(t, datetime.now(UTC) - STREAM_LAG)))
            for x in X:
                if x["id"] not in sent:
                    sent[x["id"]] = x.pop("fetched_at")
                    yield f"event: observation\ndata: {json.dumps(x, default=str)}\n\n"
            sent = {k: v for k, v in sent.items() if v > datetime.now(UTC) - STREAM_LAG}
            await asyncio.sleep(5)

    return StreamingResponse(events(), media_type="text/event-stream", headers=NO_CACHE)


def app(collect=True):
    """The ASGI app; with collect=True the scheduler polls every source by cadence."""
    @asynccontextmanager
    async def life(_):
        stop = threading.Event()
        if collect:
            threading.Thread(target=cycle.schedule, args=(stop,), daemon=True).start()
        yield
        stop.set()

    return Starlette(lifespan=life, routes=[
        Route("/fetch", fetch), Route("/api/telegram", telegram),
        Route("/api/sources", sources), Route("/api/indicators", indicators),
        Route("/api/products", products),
        Route("/api/context", context), Route("/api/signals", signals),
        Route("/regions", regions_), Route("/observations", observations),
        Route("/events", events), Route("/tracks", tracks), Route("/sources", sources),
        Route("/stream", stream),
    ])
