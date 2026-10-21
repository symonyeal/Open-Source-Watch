from datetime import UTC, datetime, timedelta
from functools import partial

from starlette.testclient import TestClient

from engine import cycle, regions, store
from engine.api import app, health
from engine.collect import rss
from tests.conftest import FIX


def fake_tasks():
    r = regions.regions()["iran-israel"]
    xml = {"rss:BBC": "bbc_rss.xml", "rss:Google News#1": "google_news.xml"}
    return [(r, sid, 300, partial(rss.parse, (FIX / f).read_bytes(), sid[4:].split("#")[0]), "rss")
            for sid, f in xml.items()]


def test_store_is_idempotent_and_empty_tables_query():
    assert store.query("SELECT count(*) n FROM event") == [{"n": 0}]
    cycle.collect(fake_tasks())
    n = store.query("SELECT count(*) n FROM raw_report")[0]["n"]
    cycle._seen = None                 # a fresh process re-reads the store
    cycle.collect(fake_tasks())
    assert n == 6 and store.query("SELECT count(*) n FROM raw_report")[0]["n"] == n
    assert store.query("SELECT count(*) n FROM collection_run")[0]["n"] == 4


def test_rebuild_reproduces_observations():
    cycle.collect(fake_tasks())
    q = "SELECT id, kind, text_en, credibility FROM observation ORDER BY id"
    before = store.query(q)
    cycle.rebuild()
    assert store.query(q) == before and len(before) == 6


def test_news_route_has_upstream_shape():
    cycle.collect(fake_tasks())
    c = TestClient(app(collect=False))
    since = (datetime.now(UTC) - timedelta(days=3650)).isoformat()
    X = c.get("/api/news", params={"conflict": "iran-israel", "since": since}).json()
    assert len(X) == 6
    assert all({"title", "link", "source", "pubDate"} <= set(x) for x in X)
    assert {x["source"] for x in X} == {"BBC", "Google News"}
    S = {s["id"]: s for s in c.get("/sources", params={"region": "iran-israel"}).json()}
    assert S["rss:BBC"]["health"] == "ok" and S["rss:BBC"]["reliability"] == "B"


def test_source_health():
    t = datetime.now(UTC)
    run = lambda s, m: {"status": s, "started_at": t - timedelta(minutes=m)}
    assert health([], 300, t) == "unknown"
    assert health([run("ok", 1)], 300, t) == "ok"
    assert health([run("ok", 20)], 300, t) == "stale"
    assert health([run("http_error", 1), run("http_error", 2), run("blocked", 3),
                   run("ok", 4)], 300, t) == "failed"
