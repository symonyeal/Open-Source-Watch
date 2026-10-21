from datetime import UTC, datetime, timedelta
from types import SimpleNamespace

from starlette.testclient import TestClient

from engine import collect, cycle, store
from engine.api import app, health
from tests.conftest import FIX

FILES = {"rss:BBC": "bbc_rss.xml", "rss:Google News#1": "google_news.xml",
         "telegram:DeepStateUA": "telegram_DeepStateUA.html", "firms:VIIRS": "firms.csv",
         "adsb:adsb.lol#mil": "adsb_mil.json", "alerts:alertsua": "alertsua_states.json",
         "drones:neptun": "neptun.json", "markets:CoinGecko": "coingecko.json"}


def offline(monkeypatch):
    """Sources of FILES only, answered from the fixtures instead of the network."""
    S = [s for s in cycle.sources() if s["id"] in FILES]
    by = {s["url"]: FILES[s["id"]] for s in S}
    get = lambda src: SimpleNamespace(
        content=(FIX / by[src["url"]]).read_bytes(), status_code=200,
        headers={"content-type": "text/plain"})
    monkeypatch.setattr(cycle, "run", lambda S, parse, runner="local":
                        collect.run(S, parse, runner, get=get))
    monkeypatch.setattr(cycle, "by_url", lambda: {s["url"]: [s] for s in S})
    return S


def count(t):
    return store.query(f"SELECT count(*) n FROM {t}")[0]["n"]


def test_store_is_idempotent_and_empty_tables_query(monkeypatch):
    assert count("event") == 0
    S = offline(monkeypatch)
    cycle.collect(S)
    n = count("raw_report")
    cycle._seen = None                 # a fresh process re-reads the store
    cycle.collect(S)
    assert n > 0 and count("raw_report") == n
    assert count("collection_run") == 2 * len(S)


def test_every_adapter_derives(monkeypatch):
    cycle.collect(offline(monkeypatch))
    K = {x["kind"] for x in store.query("SELECT DISTINCT kind FROM observation")}
    assert {"thermal_anomaly", "air_raid_alert", "airspace_activity"} <= K
    assert count("track_point") > 0


def test_rebuild_reproduces_derived_tables(monkeypatch):
    cycle.collect(offline(monkeypatch))
    q = "SELECT id, kind, text_en, credibility FROM observation ORDER BY id"
    before, tp = store.query(q), count("track_point")
    cycle.rebuild()
    assert store.query(q) == before and count("track_point") == tp


def test_compaction_keeps_rows(monkeypatch):
    S = offline(monkeypatch)
    cycle.collect(S)
    cycle._seen = None
    cycle.collect(S)
    n = count("collection_run")
    tomorrow = (datetime.now(UTC) + timedelta(days=1)).date().isoformat()
    assert store.compact(before=tomorrow) > 0 and count("collection_run") == n


def test_proxy_serves_what_was_stored(monkeypatch):
    S = offline(monkeypatch)
    url = next(s["url"] for s in S if s["id"] == "rss:BBC")
    c = TestClient(app(collect=False))
    r = c.get("/fetch", params={"url": url})
    assert r.status_code == 200 and r.content == (FIX / "bbc_rss.xml").read_bytes()
    assert count("raw_report") == 3                       # collected on demand, stored
    assert c.get("/fetch", params={"url": "https://example.com/x"}).status_code == 502


def test_telegram_route_has_upstream_shape(monkeypatch):
    cycle.collect(offline(monkeypatch))
    c = TestClient(app(collect=False))
    since = (datetime.now(UTC) - timedelta(days=3650)).isoformat()
    d = c.get("/api/telegram", params={"conflict": "russia-ukraine", "since": since}).json()
    assert set(d) == {"posts", "channels", "updated"}
    assert [p["postId"] for p in d["posts"]] == [23913, 23912, 23911]
    assert set(d["posts"][0]) == {"channel", "channelLabel", "color", "postId", "text", "date",
                                  "url"}


def test_sources_view(monkeypatch):
    cycle.collect(offline(monkeypatch))
    c = TestClient(app(collect=False))
    S = {s["id"]: s for s in c.get("/sources", params={"region": "iran-israel"}).json()}
    assert S["rss:BBC"]["health"] == "ok" and S["rss:BBC"]["reliability"] == "B"
    assert S["firms:VIIRS"]["reliability"] == "A" and S["rss:CNN"]["health"] == "unknown"


def test_source_health():
    t = datetime.now(UTC)
    run = lambda s, m: {"status": s, "started_at": t - timedelta(minutes=m)}
    assert health([], 300, t) == "unknown"
    assert health([run("ok", 1)], 300, t) == "ok"
    assert health([run("ok", 20)], 300, t) == "stale"
    assert health([run("http_error", 1), run("http_error", 2), run("blocked", 3),
                   run("ok", 4)], 300, t) == "failed"
