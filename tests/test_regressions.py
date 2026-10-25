"""Collection isolation and missing-evidence regressions from the dashboard audit."""
from datetime import UTC, datetime, timedelta

from starlette.testclient import TestClient

from engine import analyze, cycle, produce, regions, store
from engine.process import gazetteer, geocode
from engine.api import app


def test_due_is_scoped_to_region():
    t = datetime.now(UTC)
    store.append("collection_run", [{"source_id": "rss:BBC", "region": "iran-israel",
        "started_at": t, "status": "ok", "n_rows": 1, "latency_ms": 1, "runner": "test"}])
    src = {"id": "rss:BBC", "region": "south-asia", "cadence": 300}
    assert cycle.due([src], t) == [src]


def test_missing_collection_is_not_absent_evidence():
    r = regions.regions()["red-sea"]
    board = analyze.board(r, {})
    i = board[0]["indicators"][0]
    assert i["count"] == 0 and i["status"] == "no data"
    assert all("RS-1a" in h["unassessed"] for h in board[1]["hypotheses"])


def test_stale_transits_are_not_current_evidence(monkeypatch):
    r = regions.regions()["red-sea"]
    monkeypatch.setattr(analyze, "transit_indicator", lambda *a: {
        "id": "RS-2a", "status": "triggered", "count": 1, "z": -3})
    board = analyze.board(r, {})
    assert board[1]["status"] == "gap"
    assert board[1]["indicators"][0]["status"] == "no data"


def test_source_health_includes_daily_baseline():
    t = datetime.now(UTC) - timedelta(hours=25)
    store.append("collection_run", [{"source_id": "firms:VIIRS#7d", "region": "global",
        "started_at": t, "status": "ok", "n_rows": 1, "latency_ms": 1, "runner": "test"}])
    src = {x["id"]: x for x in cycle.source_table(regions.regions()["red-sea"])}
    assert src["firms:VIIRS#7d"]["health"] == "ok"


def test_api_rejects_invalid_filters():
    c = TestClient(app(collect=False))
    for path in ("/events?region=typo", "/events?limit=-1", "/observations?limit=oops",
                 "/tracks?since=garbage", "/events?limit=10001"):
        assert c.get(path).status_code == 400, path


def test_constant_transit_baseline_serializes_as_json(monkeypatch):
    t = datetime.now(UTC)
    last = t.date() - timedelta(days=1)
    days = {last - timedelta(days=k) for k in range(15)}
    counts = {d: 10 for d in days}
    counts[last] = 1
    monkeypatch.setattr(analyze, "daily_transits", lambda *a: (counts, days))
    monkeypatch.setattr(cycle, "source_table", lambda r: [{
        "id": "ais:aisstream", "health": "ok"}])
    res = TestClient(app(collect=False)).get("/api/indicators?region=red-sea")
    assert res.status_code == 200
    i = res.json()[1]["indicators"][0]
    assert i["status"] == "triggered" and i["z"] is None
    assert i["baseline_sd"] == 0


def test_product_confidence_uses_the_indicator_window():
    t = datetime.now(UTC)
    store.append("event", [{"id": f"ship-{k}", "region": "red-sea", "kind": "maritime_incident",
        "t_start": t - timedelta(hours=48), "t_end": t - timedelta(hours=48), "credibility": 2,
        "observation_ids": [f"o-{k}"], "n_independent": 2, "lat": 14, "lon": 43}
        for k in range(3)])
    r = regions.regions()["red-sea"]
    board = analyze.board(r, {"rss:BBC": "ok"}, t)
    text = produce.report(r, board, [], "sitrep", t)
    assert "(RS-1a); high confidence" in text


def test_south_asia_gazetteer_includes_places_beyond_curated_capitals():
    G = gazetteer(regions.regions()["south-asia"])
    for title, name in (("Flooding reported in Khulna", "Khulna"),
                        ("Protests in Lahore", "Lahore"),
                        ("Fighting near Mandalay", "Mandalay")):
        lat, lon, precision, names = geocode(G, title)
        assert precision == "city" and name in names
        assert lat is not None and lon is not None


def test_south_asia_reporting_counts_cannot_trigger_an_uncalibrated_alert():
    t = datetime.now(UTC)
    store.append("event", [{"id": f"sa-{k}", "region": "south-asia",
        "kind": "maritime_incident", "t_start": t - timedelta(hours=1),
        "t_end": t - timedelta(hours=1), "credibility": 3, "observation_ids": [f"o-{k}"],
        "n_independent": 1, "lat": 22.3, "lon": 91.8} for k in range(20)])
    r = regions.regions()["south-asia"]
    board = analyze.board(r, {"rss:BBC Asia": "ok"}, t)
    assert board[0]["status"] == "open"
    i = board[0]["indicators"][0]
    assert i["count"] == 20 and i["status"] == "observed"
    assert "watch" not in i and "trigger" not in i
    assert "no calibrated alert threshold" in produce.report(r, board, [], "sitrep", t)
