"""Engine HTTP API (spec § Seam).

/fetch?url=U   the recording proxy: the latest stored response for an upstream URL
               (src/lib/fetcher.ts forwards here when ENGINE_URL is set)
/api/telegram  upstream's TelegramData shape (src/lib/types/api.ts), from the store
/api/indicators PIR and indicator status of a region
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
from starlette.responses import JSONResponse, Response, StreamingResponse
from starlette.routing import Route

from engine import analyze, cycle, regions, store

NO_CACHE = {"Cache-Control": "no-cache, no-store, must-revalidate"}


def region_of(req):
    rid = req.query_params.get("region") or req.query_params.get("conflict")
    R = regions.regions()
    return R.get(rid) or R["iran-israel"]


def since(req, hours=48):
    s = req.query_params.get("since")
    return datetime.fromisoformat(s) if s else datetime.now(UTC) - timedelta(hours=hours)


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
        + ([q["source"] + "%"] if q.get("source") else []) + [int(q.get("limit", 500))]
    return js(await run_in_threadpool(store.query, sql, p))


async def events(req):
    r, q = region_of(req), req.query_params
    sql = ("SELECT * FROM event WHERE region = ? AND t_end > ?"
           + (" AND kind = ?" if q.get("kind") else "") + " ORDER BY t_end DESC LIMIT ?")
    p = [r["id"], since(req)] + ([q["kind"]] if q.get("kind") else []) + [int(q.get("limit", 500))]
    return js(await run_in_threadpool(store.query, sql, p))


async def tracks(req):
    r = region_of(req)
    return js(await run_in_threadpool(
        store.query, "SELECT * FROM track_point WHERE region = ? AND t > ? ORDER BY entity_id, t",
        (r["id"], since(req, 6))))


def indicator_board(r):
    return analyze.board(r, {s["id"]: s["health"] for s in source_table(r)})


async def indicators(req):
    return js(await run_in_threadpool(indicator_board, region_of(req)))


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
    t = datetime.now(UTC)
    runs = defaultdict(list)
    for x in store.query("SELECT * FROM collection_run WHERE region IN (?, 'global') "
                         "AND started_at > ? ORDER BY started_at DESC",
                         (r["id"], t - timedelta(days=1))):
        runs[x["source_id"]].append(x)
    return [{"id": s["id"], "region": s["region"], "url": s["url"],
             "reliability": regions.reliability(r, s["id"]), "owner": regions.owner(r, s["id"]),
             "cadence_s": s["cadence"], "health": health(runs[s["id"]], s["cadence"], t),
             "last_status": runs[s["id"]][0]["status"] if runs[s["id"]] else None,
             "last_run": runs[s["id"]][0]["started_at"] if runs[s["id"]] else None}
            for s in cycle.sources([r["id"], "global"])]


async def sources(req):
    return js(await run_in_threadpool(source_table, region_of(req)))


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
    """Server-sent events: each new observation of the region, polled every 5 s."""
    r = region_of(req)

    async def events():
        t = datetime.now(UTC)
        while not await req.is_disconnected():
            X = await run_in_threadpool(
                store.query, "SELECT o.* FROM observation o JOIN raw_report r ON r.id = o.raw_id "
                "WHERE o.region = ? AND r.fetched_at > ? ORDER BY o.t", (r["id"], t))
            t = datetime.now(UTC) - timedelta(seconds=1)
            for x in X:
                yield f"event: observation\ndata: {json.dumps(x, default=str)}\n\n"
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
        Route("/regions", regions_), Route("/observations", observations),
        Route("/events", events), Route("/tracks", tracks), Route("/sources", sources),
        Route("/stream", stream),
    ])
