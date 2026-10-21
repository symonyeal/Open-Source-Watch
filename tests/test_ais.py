"""AIS stream adapter (engine.collect.ais) and its storage path (cycle.ingest).
The fixture follows aisstream.io's documented message format (no key to record a
live message)."""
from datetime import UTC, datetime

from engine import cycle, fuse, regions, store
from engine.collect import ais
from tests.conftest import FIX

MSG = (FIX / "aisstream_position_reports.jsonl").read_text(encoding="utf8").splitlines()
SRC = ais.sources(regions.regions())[0]


def count(t):
    return store.query(f"SELECT count(*) n FROM {t}")[0]["n"]


def test_positions_only_and_nanosecond_times():
    X = ais.parse("\n".join(MSG).encode(), SRC)
    assert [x["payload"]["mmsi"] for x in X] == [538009876, 636012345]
    assert X[0]["published_at"] == datetime(2026, 10, 21, 8, 0, 1, 123456, tzinfo=UTC)
    assert X[0]["payload"]["name"] == "EXAMPLE TANKER"


def test_subscription_covers_every_gated_region():
    s = ais.subscription("k", regions.regions())
    assert len(s["BoundingBoxes"]) == sum(bool(r["gate"]) for r in regions.regions().values())
    assert s["FilterMessageTypes"] == ["PositionReport"]


def test_without_key_the_stream_is_blocked(monkeypatch):
    monkeypatch.delenv("AISSTREAM_KEY", raising=False)
    [row] = cycle.ingest(SRC, MSG)
    assert row["status"] == "blocked" and count("raw_report") == 0


def test_with_key_messages_become_vessel_tracks(monkeypatch):
    monkeypatch.setenv("AISSTREAM_KEY", "test")
    [row] = cycle.ingest(SRC, MSG)
    assert row["status"] == "ok" and row["n_rows"] == 2
    T = store.query("SELECT entity_id, region FROM track_point ORDER BY entity_id")
    assert {(x["entity_id"], x["region"]) for x in T} >= \
        {("mmsi:538009876", "red-sea"), ("mmsi:636012345", "hormuz")}
    fuse.fuse()
    assert store.query("SELECT count(*) n FROM entity WHERE type = 'vessel'")[0]["n"] == 2
    assert cycle.ingest(SRC, [])[0]["status"] == "empty"
