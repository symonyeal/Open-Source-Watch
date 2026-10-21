"""Engine HTTP API (spec § Seam).

Engine-native routes: /regions, /observations, /sources, /stream.
Upstream-shaped routes under /api/* return exactly the JSON of the IRONSIGHT
route of the same path (src/lib/types/api.ts), built from the store, so a
Next.js route with ENGINE_URL set can forward to them unchanged.
"""
import asyncio
import json
import threading
from contextlib import asynccontextmanager
from datetime import UTC, datetime, timedelta

from starlette.applications import Starlette
from starlette.concurrency import run_in_threadpool
from starlette.responses import JSONResponse, StreamingResponse
from starlette.routing import Route

from engine import cycle, regions, store

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


def health(runs, cadence, t):
    """ok | stale | failed | unknown (spec § Errors)."""
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
    runs = store.query("SELECT * FROM collection_run WHERE region = ? AND started_at > ? "
                       "ORDER BY started_at DESC", (r["id"], t - timedelta(days=1)))
    out = []
    for _, sid, c, _, a in cycle.tasks([r["id"]]):
        R = [x for x in runs if x["source_id"] == sid]
        out.append({"id": sid, "adapter": a, "reliability": regions.reliability(r, sid),
                    "owner": regions.owner(r, sid), "cadence_s": c, "health": health(R, c, t),
                    "last_status": R[0]["status"] if R else None,
                    "last_run": R[0]["started_at"] if R else None})
    return out


async def sources(req):
    return js(await run_in_threadpool(source_table, region_of(req)))


def news_items(r, t):
    """Upstream NewsItem[]: deduplicated on the first 60 lowercased title characters,
    nearest to now first, at most 100 (src/app/api/news/route.ts)."""
    X = store.query(
        "SELECT o.text_en, o.t, r.payload FROM observation o JOIN raw_report r ON r.id = o.raw_id "
        "WHERE o.region = ? AND o.source_id LIKE 'rss:%' AND o.t > ?", (r["id"], t))
    now, seen, out = datetime.now(UTC), set(), []
    for x in sorted(X, key=lambda x: abs((now - x["t"]).total_seconds())):
        k = x["text_en"].lower().strip()[:60]
        if k in seen:
            continue
        seen.add(k)
        p = json.loads(x["payload"])
        item = {"title": x["text_en"], "link": p["link"], "source": p["source"],
                "pubDate": p["pubDate"]}
        out.append(item | ({"category": p["category"]} if p["category"] else {}))
    return out[:100]


async def news(req):
    return js(await run_in_threadpool(news_items, region_of(req), since(req)))


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
        Route("/regions", regions_), Route("/observations", observations),
        Route("/sources", sources), Route("/stream", stream), Route("/api/news", news),
    ])
