"""Collection, stage 2 of the cycle (spec § Intelligence cycle).

A source is a dict {id, region, url, cadence, headers?, ...}; `region` is a
region id or "global". An adapter module exposes

    sources(R)       -> list of sources, R the region registry
    parse(body, src) -> list of items {native_id, published_at, url, payload}
    fetch(src)       -> response with `content`; optional, for a body assembled
                        from several requests (default: GET src["url"])

`run` fetches one URL and parses it for every source sharing that URL. A failed
fetch or parse is the run's status, never an exception (spec § Errors).

A payload's format belongs to its adapter: the adapter writes it (parse), and its
processor (engine.process) and its readers read it. Other modules decode no
payload: they read raw reports through `stored`, or through the adapter's reader
where its rule picks among the stored reports (mnd.reports, ioda.events,
ucdp.events, context.releases, rss.outlets).
"""
import hashlib
import json
import time
from datetime import UTC, datetime
from types import SimpleNamespace
from xml.etree.ElementTree import ParseError

import httpx

from engine import store

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Open-Source-Watch/0.1"
EXCERPT = 280  # characters of article or post text kept (spec § Data model)


def now():
    return datetime.now(UTC)


def epoch(ms):
    return datetime.fromtimestamp(ms / 1000, UTC)


def adapter(source_id):
    """Adapter of a source id "<adapter>:<name>[#k]"."""
    return source_id.split(":", 1)[0]


def raw_id(region, source_id, native_id):
    """Content hash of one item; regions sharing a source keep separate reports."""
    return hashlib.sha256(f"{region}\0{source_id}\0{native_id}".encode()).hexdigest()[:32]


def fetch(src, timeout=10.0):
    """GET src["url"]; raises httpx.HTTPStatusError on status >= 400."""
    res = httpx.get(src["url"], timeout=timeout, follow_redirects=True,
                    headers={"User-Agent": UA, **src.get("headers", {})})
    res.raise_for_status()
    return res


def response(obj):
    """Response-like result of a fetch that assembles several requests into one
    body: obj as JSON, read by run() as `content`."""
    return SimpleNamespace(content=json.dumps(obj).encode())


def status_of(e):
    if isinstance(e, httpx.HTTPStatusError):
        return "blocked" if e.response.status_code in (401, 403, 429) else "http_error"
    if isinstance(e, httpx.HTTPError):
        return "http_error"
    return "parse_error"


ERRORS = (httpx.HTTPError, ParseError, ValueError, KeyError, TypeError, IndexError,
          AttributeError)


def run(S, parse, runner="local", get=fetch):
    """One poll of the URL shared by sources S.
    Returns (response or None, raw_report rows, collection_run rows)."""
    t0, started = time.monotonic(), now()
    try:
        res, err = get(S[0]), None
    except ERRORS as e:
        res, err = None, e
    raw, runs = [], []
    for src in S:
        try:
            if err:
                raise err
            items = parse(res.content, src)
            status = "ok" if items else "empty"
        except ERRORS as e:
            items, status = [], status_of(e)
        rows = [{
            "id": raw_id(src["region"], src["id"], it["native_id"]), "source_id": src["id"],
            "region": src["region"], "fetched_at": started,
            "published_at": it.get("published_at"), "url": it.get("url"),
            "payload": json.dumps(it["payload"], ensure_ascii=False, default=str),
        } for it in items]
        raw += rows
        runs.append({"source_id": src["id"], "region": src["region"], "started_at": started,
                     "status": status, "n_rows": len(rows),
                     "latency_ms": int(1000 * (time.monotonic() - t0)), "runner": runner})
    return res, raw, runs


def decode(x):
    """Raw report row x with the payload run() encoded as JSON decoded."""
    return x | {"payload": json.loads(x["payload"])}


def stored(*A, regions=None, since=None):
    """Stored raw reports of adapters A and region ids `regions` (every adapter or
    region if none), dated at or after `since` (published_at, or fetched_at without
    one, as an observation's t); decoded, in fetch order."""
    W, p = ["true"], []
    if A:
        W.append("(" + " OR ".join(["source_id LIKE ?"] * len(A)) + ")")
        p += [f"{a}:%" for a in A]
    if regions:
        W.append("region IN (" + ", ".join(["?"] * len(regions)) + ")")
        p += list(regions)
    if since is not None:
        W.append("coalesce(published_at, fetched_at) >= ?")
        p.append(since)
    return [decode(x) for x in store.query(
        f"SELECT * FROM raw_report WHERE {' AND '.join(W)} ORDER BY fetched_at, id", p)]
