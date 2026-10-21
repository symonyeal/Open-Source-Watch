"""Collection, stage 2 of the cycle (spec § Intelligence cycle).

An adapter module exposes

    sources(r) -> iterable of (source_id, cadence_s, fetch)
    fetch()    -> list of items {native_id, published_at, url, payload}

`run` turns one fetch into immutable raw reports and one collection_run row. A
failed fetch is the run's status, never an exception (spec § Errors).
"""
import hashlib
import json
import time
from datetime import UTC, datetime
from xml.etree.ElementTree import ParseError

import httpx

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Open-Source-Watch/0.1"
EXCERPT = 280  # characters of article or post text kept (spec § Data model)


def now():
    return datetime.now(UTC)


def get(url, timeout=8.0, **kw):
    """GET with the engine's user agent; raises on HTTP status >= 400."""
    headers = {"User-Agent": UA, **kw.pop("headers", {})}
    res = httpx.get(url, timeout=timeout, follow_redirects=True, headers=headers, **kw)
    res.raise_for_status()
    return res


def raw_id(source_id, native_id):
    return hashlib.sha256(f"{source_id}\0{native_id}".encode()).hexdigest()[:32]


def run(region, source_id, fetch, runner="local"):
    """One poll of one source. Returns (raw_report rows, collection_run row)."""
    t0, started = time.monotonic(), now()
    try:
        items = fetch()
        status = "ok" if items else "empty"
    except httpx.HTTPStatusError as e:
        items = []
        status = "blocked" if e.response.status_code in (401, 403, 429) else "http_error"
    except httpx.HTTPError:
        items, status = [], "http_error"
    except (ParseError, ValueError, KeyError, TypeError, IndexError):
        items, status = [], "parse_error"
    raw = [{
        "id": raw_id(source_id, it["native_id"]), "source_id": source_id, "region": region,
        "fetched_at": started, "published_at": it.get("published_at"), "url": it.get("url"),
        "payload": json.dumps(it["payload"], ensure_ascii=False, default=str),
    } for it in items]
    row = {
        "source_id": source_id, "region": region, "started_at": started, "status": status,
        "n_rows": len(raw), "latency_ms": int(1000 * (time.monotonic() - t0)), "runner": runner,
    }
    return raw, row
