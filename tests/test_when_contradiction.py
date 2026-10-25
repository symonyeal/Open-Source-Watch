"""Event-time extraction (engine.process.when), the contradiction rule
(engine.fuse.contradicted) and the thermal persistence rule (process.feeds)."""
from collections import defaultdict
from datetime import UTC, date, datetime, timedelta

import pytest

from engine import fuse, regions
from engine.process import feeds
from engine.process.when import event_time

PUB = datetime(2026, 10, 21, 7, 21, 53, tzinfo=UTC)   # 10:21 in Kyiv (UTC+3)
KYIV = "An air raid alert sounded in Kyiv at around 5:20 a.m. local time on Oct. 21"


@pytest.mark.parametrize("s, want", [
    (KYIV, datetime(2026, 10, 21, 2, 20, tzinfo=UTC)),
    ("Explosions heard at 23:40 local time", datetime(2026, 10, 20, 20, 40, tzinfo=UTC)),
    ("Sirens at 11 pm", datetime(2026, 10, 20, 20, 0, tzinfo=UTC)),       # day before
    ("overnight attack on the port", None),
    ("Talks at 3 p.m. on Nov. 2", None),                                  # announced
    ("A 5 member team", None),
    ("13 pm", None),
])
def test_event_time(s, want):
    assert event_time(s, PUB, "Europe/Kyiv") == want


def test_event_time_across_new_year():
    jan2 = datetime(2027, 1, 2, 9, 0, tzinfo=UTC)
    assert event_time("Strike at 4 a.m. on Dec. 31", jan2, "Europe/Kyiv") == \
        datetime(2026, 12, 31, 2, 0, tzinfo=UTC)


@pytest.mark.parametrize("s", ["Strike at 5 p.m. on Feb. 30", "at 5 pm on Oct 45",
                               "Sirens at 11 pm on Feb 29", "Explosion at 0 pm", "at 13 pm"])
def test_impossible_time_is_none_not_an_error(s):
    """Regression: an impossible date raised ValueError and stopped the poll's processing;
    "0 pm" was read as noon."""
    assert event_time(s, PUB, "Europe/Kyiv") is None


R = regions.regions()["russia-ukraine"]


def claim(tau):
    return {"source_id": "telegram:KyivIndependent_official", "t_event": tau}


def runs(t0, n, status="ok"):
    return [{"source_id": "alerts:alertsua", "started_at": t0 + timedelta(minutes=k),
             "status": status} for k in range(n)]


def test_kyiv_report_of_an_alert_before_collection_is_not_contradicted():
    """The live false positive of 2026-10-21: no warning-source runs at 02:20 UTC."""
    tau = event_time(KYIV, PUB, "Europe/Kyiv")
    assert not fuse.contradicted(R, [claim(tau)], [], runs(PUB - timedelta(minutes=20), 40))


def test_covered_silence_contradicts():
    tau = datetime(2026, 10, 21, 3, 0, tzinfo=UTC)
    assert fuse.contradicted(R, [claim(tau)], [], runs(tau - timedelta(minutes=5), 10))


@pytest.mark.parametrize("M, alerts, status", [
    ([claim(None)], [], "ok"),                                               # no stated time
    ([claim(datetime(2026, 10, 21, 3, 0, tzinfo=UTC))],
     [datetime(2026, 10, 21, 3, 20, tzinfo=UTC)], "ok"),                     # alert recorded
    ([claim(datetime(2026, 10, 21, 3, 0, tzinfo=UTC))], [], "http_error"),   # source failing
    ([claim(datetime(2026, 10, 21, 3, 0, tzinfo=UTC)),
      {"source_id": "alerts:alertsua", "t_event": None}], [], "ok"),         # warning supports
])
def test_no_contradiction(M, alerts, status):
    tau = datetime(2026, 10, 21, 3, 0, tzinfo=UTC)
    assert not fuse.contradicted(R, M, alerts, runs(tau - timedelta(minutes=5), 10, status))


def test_persistent_heat_source():
    """Active on >= 3 of the 7 prior days: persistent; a new cell is an anomaly."""
    D = defaultdict(set)
    d0 = date(2026, 10, 21)
    c = feeds.cell(29.5, 47.7)
    D[c] = {d0 - timedelta(days=k) for k in (1, 3, 6)}
    assert feeds.persistent(D, c, d0)
    assert not feeds.persistent(D, c, d0 + timedelta(days=5))     # 1 of the prior 7 days
    assert not feeds.persistent(D, feeds.cell(31.0, 35.0), d0)
