"""Transit counting, ACH ranking and products (engine.analyze, engine.produce)."""
from datetime import UTC, datetime, timedelta

from engine import analyze, produce, regions, store

R = regions.regions()
RS = R["red-sea"]
GATE = RS["gate"]["bab-el-mandeb"]["line"]


def test_segment_crossing():
    a, b = GATE
    assert analyze.crosses((12.70, 43.30), (12.50, 43.45), a, b)       # through the strait
    assert not analyze.crosses((13.5, 42.5), (13.6, 42.6), a, b)        # far north
    assert not analyze.crosses(a, (12.0, 44.0), a, b)                   # touching is not crossing


def test_transit_gap_over_policy_limit_is_not_counted():
    a, b = GATE
    P = [
        {"t": datetime(2026, 10, 1, tzinfo=UTC), "lat": 12.70, "lon": 43.30},
        {"t": datetime(2026, 10, 2, tzinfo=UTC), "lat": 12.50, "lon": 43.45},
    ]
    assert analyze.crosses((P[0]["lat"], P[0]["lon"]),
                           (P[1]["lat"], P[1]["lon"]), a, b)
    assert analyze.crossings(P, GATE) == []
    assert analyze.crossings(P, GATE, timedelta(days=1)) == [P[1]["t"]]
    P[1]["t"] = P[0]["t"]
    assert analyze.crossings(P, GATE) == []


def vessel(mmsi, day, crossings):
    """Track points of a vessel that crosses the gate `crossings` times on `day`."""
    t = datetime.combine(day, datetime.min.time(), UTC) + timedelta(hours=1)
    P, south = [], True
    for k in range(crossings + 1):
        lat, lon = (12.50, 43.45) if south else (12.70, 43.30)
        P.append({"entity_id": f"mmsi:{mmsi}", "region": "red-sea", "t": t + timedelta(minutes=k),
                  "lat": lat, "lon": lon, "heading": None, "speed": None,
                  "source_id": "ais:aisstream"})
        south = not south
    return P


def test_transit_indicator_detects_a_collapse():
    t = datetime(2026, 10, 21, 12, tzinfo=UTC)
    rows = []
    for k in range(1, 16):                  # 14 prior days of 10 vessels, then 2 vessels
        n = 2 if k == 1 else 10 + (k % 3)
        day = (t - timedelta(days=k)).date()
        rows += [p for v in range(n) for p in vessel(1000 * k + v, day, 1)]
    store.append("track_point", rows)
    i = next(i for p in RS["pir"] for i in p["indicator"] if i.get("metric") == "transit")
    x = analyze.transit_indicator(RS, i, t)
    assert x["count"] == 2 and x["z"] < -2 and x["status"] == "triggered"


def test_transit_indicator_without_history_has_no_data():
    i = next(i for p in RS["pir"] for i in p["indicator"] if i.get("metric") == "transit")
    x = analyze.transit_indicator(RS, i, datetime(2026, 10, 21, tzinfo=UTC))
    assert x["status"] == "no data"


def test_isolated_ais_points_do_not_count_as_sampled_coverage():
    t = datetime(2026, 10, 21, 12, tzinfo=UTC)
    store.append("track_point", [
        {"entity_id": f"mmsi:{k}", "region": "red-sea", "t": t - timedelta(days=k),
         "lat": 12.60, "lon": 43.35, "heading": None, "speed": None,
         "source_id": "ais:aisstream"}
        for k in range(1, 9)
    ])
    i = next(i for p in RS["pir"] for i in p["indicator"] if i.get("metric") == "transit")
    x = analyze.transit_indicator(RS, i, t)
    assert x["count"] == 0 and x["status"] == "no data"


def test_ach_ranks_by_inconsistency():
    p = next(p for p in RS["pir"] if p["id"] == "RS-2")
    I = [{"id": "RS-1a", "status": "triggered"}, {"id": "RS-2a", "status": "watch"}]
    H = analyze.ach(p, I)
    assert H[0]["id"] == "RS-2-H1" and H[0]["inconsistent"] == []
    assert {h["id"]: len(h["inconsistent"]) for h in H} == \
        {"RS-2-H1": 0, "RS-2-H2": 1, "RS-2-H3": 2}
    I = [{"id": "RS-1a", "status": "inactive"}, {"id": "RS-2a", "status": "no data"}]
    H = analyze.ach(p, I)
    assert H[0]["unassessed"] == ["RS-2a"]


def test_intsum_reports_each_pir_and_cites_its_standards():
    r = R["iran-israel"]
    B = analyze.board(r, {})
    text = produce.report(r, B, [{"id": "rss:BBC", "health": "failed"}], "intsum")
    assert all(f"**{p['id']}**" in text for p in r["pir"])
    assert "Collection gap" in text or "collection gap" in text
    assert "0 of 1 sources healthy" in text
    assert produce.confidence(2) == "high" and produce.confidence(3) == "moderate"
    assert produce.product(r, "sitrep", text)["period_h"] == 6
