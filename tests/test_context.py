"""Recorded context: provider errors, revisions, missing days and dated comparisons."""
import json
from datetime import UTC, datetime, timedelta
from types import SimpleNamespace

import pytest
from starlette.testclient import TestClient

from engine import collect, context, cycle, regions, store
from engine.api import app
from engine.collect import context as feeds
from tests.conftest import FIX

T = datetime(2026, 10, 24, tzinfo=UTC)


def test_recorded_provider_responses():
    """Public responses captured 2026-10-24; provenance in docs/REVIEW-LOG.md."""
    S = feeds.sources(regions.regions())
    for filename, code in (("fred_brent.csv", "DCOILBRENTEU"),
                           ("wb_energy_imports.json", "EG.IMP.CONS.ZS")):
        src = next(s for s in S if s["code"] == code)
        [item] = feeds.parse((FIX / filename).read_bytes(), src)
        assert item["payload"]["points"]
        assert any(p["value"] is not None for p in item["payload"]["points"])
    src = next(s for s in S if s["code"] == "port241")
    # The small live probe deliberately requested five rows; its transfer-limit
    # flag must prevent that sample from masquerading as a complete history.
    with pytest.raises(ValueError, match="incomplete page"):
        feeds.parse((FIX / "portwatch_chittagong.json").read_bytes(), src)


def test_fred_keeps_missing_and_negative_prices():
    src = next(s for s in feeds.sources(regions.regions()) if s.get("code") == "DCOILWTICO")
    [item] = feeds.parse(b"observation_date,DCOILWTICO\n2026-10-19,-2.5\n2026-10-20,.\n", src)
    assert item["payload"]["points"] == [
        {"date": "2026-10-19", "value": -2.5}, {"date": "2026-10-20", "value": None}]
    assert item["published_at"] == datetime(2026, 10, 20, tzinfo=UTC)
    assert item["payload"]["unit"] == "USD/bbl"


def test_provider_errors_and_truncation_are_collection_failures():
    src = feeds.port_source("port241", "Chattogram", "south-asia", "port", "2026-09-01")
    for body in (b'{"error":{"code":400}}', b'{"features":[],"exceededTransferLimit":true}'):
        _, rows, [run] = collect.run([src], feeds.parse,
                                    get=lambda _, body=body: SimpleNamespace(content=body))
        assert rows == [] and run["status"] == "parse_error"


def test_portwatch_accepts_current_and_legacy_dates():
    src = feeds.port_source("port241", "Chattogram", "south-asia", "port", "2026-09-01")
    d = {"features": [{"attributes": {"date": "2026-10-18", "portid": "port241",
         "portname": "Chittagong", "portcalls": 8, "portcalls_tanker": 0}}]}
    [item] = feeds.parse(json.dumps(d).encode(), src)
    assert item["payload"]["points"] == [{"date": "2026-10-18", "value": 8, "tanker": 0}]
    assert feeds.day(1792281600000) == "2026-10-18"


def test_centered_seven_day_mean_requires_calendar_coverage():
    P = [{"date": (T.date() - timedelta(days=k)).isoformat(), "value": 5 if k <= 7 else 10}
         for k in range(7, 0, -1)]
    assert context.mean7(P) == {"mean": 5, "n": 7, "center_date": "2026-10-20",
                                "window_start": "2026-10-17", "window_end": "2026-10-23"}
    assert context.mean7(P[1:]) is None
    assert context.mean7([p | {"value": 0} for p in P])["mean"] == 0


def test_wdi_uses_country_specific_years_and_preserves_exporters():
    src = {"kind": "wdi", "code": "EG.IMP.CONS.ZS", "name": "Net energy imports",
           "countries": ["BGD", "MMR"]}
    d = [{"pages": 1}, [
        {"countryiso3code": "BGD", "country": {"value": "Bangladesh"}, "date": "2024", "value": 20},
        {"countryiso3code": "MMR", "country": {"value": "Myanmar"}, "date": "2024", "value": None},
        {"countryiso3code": "MMR", "country": {"value": "Myanmar"}, "date": "2022", "value": -5}]]
    [item] = feeds.parse(json.dumps(d).encode(), src)
    out = context.series(item["payload"], T, "ok", T)
    assert out["countries"]["BGD"]["date"] == "2024"
    assert out["countries"]["MMR"]["value"] == -5
    assert out["countries"]["MMR"]["date"] == "2022"
    d[0]["pages"] = 2
    with pytest.raises(ValueError):
        feeds.parse(json.dumps(d).encode(), src)


def test_context_api_selects_latest_release_and_aligns_spread_dates():
    for code, vals in (("DCOILBRENTEU", [100, 105]), ("DCOILWTICO", [90, None])):
        for rev, delta in enumerate((0, 1)):
            payload = {"kind": "fred", "code": code, "name": code,
                       "source": "EIA via FRED", "source_url": "https://fred.stlouisfed.org/",
                       "unit": "USD/bbl", "points": [
                           {"date": "2026-10-19", "value": vals[0] + delta},
                           {"date": "2026-10-20", "value": vals[1]}]}
            store.append("raw_report", [{"id": f"{code}-{rev}", "source_id": f"context:FRED#{code}",
                         "region": "global", "fetched_at": T + timedelta(seconds=rev),
                         "payload": json.dumps(payload)}])
    d = context.view(regions.regions()["south-asia"], T)
    assert d["spread"] == {"date": "2026-10-19", "value": 10, "unit": "USD/bbl"}
    wti = next(s for s in d["series"] if s["code"] == "DCOILWTICO")
    assert wti["latest"] == {"date": "2026-10-19", "value": 91}
    res = TestClient(app(collect=False)).get("/api/context?region=south-asia")
    assert res.status_code == 200 and len(res.json()["countries"]) == 8


def test_context_collection_reaches_recorded_view(monkeypatch):
    src = next(s for s in cycle.sources(adapters=["context"]) if s.get("code") == "DCOILWTICO")
    monkeypatch.setattr(cycle, "run", lambda S, parser, runner="local": collect.run(
        S, parser, runner, get=lambda _: SimpleNamespace(status_code=200, headers={},
            content=b"observation_date,DCOILWTICO\n2026-10-19,90\n2026-10-20,91\n")))
    assert cycle.collect([src])[0]["status"] == "ok"
    d = context.view(regions.regions()["south-asia"], T)
    assert len(d["series"]) == 1 and d["series"][0]["latest"]["value"] == 91
    assert d["series"][0]["health"] == "ok"


def test_future_and_missing_values_do_not_become_current_prices():
    payload = {"kind": "fred", "points": [{"date": "2026-10-25", "value": 9},
        {"date": "2026-10-23", "value": None}], "code": "x"}
    s = context.series(payload, T, "ok", T)
    assert s["latest"] is None and s["age_days"] is None
