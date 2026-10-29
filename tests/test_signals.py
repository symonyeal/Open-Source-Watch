"""Signals: GNSS cells (kernel, gpsjam, own ADS-B), Taiwan MND reports, IODA outages,
UCDP reference events and the recall grid. Fixtures are public responses captured
2026-10-29 and trimmed (docs/REVIEW-LOG.md)."""
import csv
import io
import json
import time
from datetime import UTC, datetime, timedelta
from types import SimpleNamespace

import numpy as np
import osw_kernels
import pytest
from starlette.testclient import TestClient

from engine import analyze, collect, cycle, fuse, gnss, regions, signals, store, validate
from engine.api import app
from engine.collect import gpsjam, integrity, ioda, mnd, ucdp
from tests.conftest import FIX

R = regions.regions()
JAM = (FIX / "gpsjam_2026-10-27_sample.csv").read_text(encoding="utf8")


def body(x):
    return lambda _: collect.response(x)


# GNSS ---------------------------------------------------------------------------

def test_kernel_cells_reproduce_gpsjam_indices():
    """Each gpsjam cell's centre maps back to the same H3 resolution-4 index."""
    h = np.array([int(r["hex"], 16) for r in csv.DictReader(io.StringIO(JAM))], np.uint64)
    lat, lon = osw_kernels.centres(h)
    assert (osw_kernels.cells(lat, lon, 4) == h).all()
    assert [len(b) for b in osw_kernels.boundaries(h[:3])] == [6, 6, 6]
    bad = osw_kernels.cells(np.array([np.nan, 91.0, 0.0]), np.array([0.0, 0.0, 181.0]), 4)
    assert bad.tolist() == [0, 0, 0]
    with pytest.raises(ValueError):
        osw_kernels.centres(np.array([12345], np.uint64))


def test_aircraft_counts_once_per_cell_day_bad_if_any_report_bad():
    day = np.array([1, 1, 1, 1, 2, 1], np.int64)
    h = np.array([7, 7, 7, 9, 7, 0], np.uint64)       # 0: no coordinate, dropped
    icao = np.array([1, 1, 2, 1, 1, 3], np.uint32)
    bad = np.array([False, True, False, False, False, True])
    D, H, G, B = osw_kernels.aircraft_counts(day, h, icao, bad)
    assert list(zip(D.tolist(), H.tolist(), G.tolist(), B.tolist())) == \
        [(1, 7, 1, 1), (1, 9, 1, 0), (2, 7, 1, 0)]


def test_gpsjam_formula_and_bands():
    """gpsjam FAQ: 100 (bad − 1) / (good + bad); < 2 % low, 2-10 % medium, > 10 % high."""
    assert gnss.pct(0, 1) == 0 and gnss.pct(10, 0) == 0 and gnss.pct(0, 0) == 0
    assert gnss.pct(98, 2) == 1.0 and gnss.band(1.99) == "low"
    assert gnss.band(2) == gnss.band(10) == "medium" and gnss.band(10.01) == "high"
    assert gnss.band(gnss.pct(8, 2)) == "medium" and gnss.band(gnss.pct(3, 3)) == "high"


def test_per_aircraft_rule():
    assert gnss.bad(8, 9, False) is False
    assert gnss.bad(8, 7, False) and gnss.bad(6, 9, False) and gnss.bad(None, None, True)
    assert gnss.bad(0, 10, False) is False                # NIC 0 with NACp ≥ 10 is unknown NIC
    assert gnss.bad(0, None, False) and gnss.bad(None, 9, False) is False


def test_gpsjam_region_cells_and_stored_days(monkeypatch):
    S = gpsjam.sources(R)
    src = next(s for s in S if s["region"] == "black-sea")
    day = lambda k: (datetime.now(UTC).date() - timedelta(days=k)).isoformat()
    d = {"manifest": {day(2): {"date": day(2), "suspect": "false", "source": "merged"}},
         "days": {day(2): JAM}}
    [item] = gpsjam.parse(json.dumps(d).encode(), src)
    p = item["payload"]
    assert item["native_id"] == day(2) and p["suspect"] is False and p["res"] == 4
    lat, lon = osw_kernels.centres(np.array([int(h, 16) for h, *_ in p["cells"]], np.uint64))
    assert p["cells"] and all(regions.in_bbox(R["black-sea"], a, o) for a, o in zip(lat, lon))
    # A day is fetched again until every region stores it; older days are not fetched.
    _, raw, _ = collect.run([src], gpsjam.parse, get=body(d))
    store.append("raw_report", raw)
    # The region's GNSS indicator counts the latest stored day's high cells.
    high = sum(gnss.band(gnss.pct(g, b)) == "high" for _, g, b in p["cells"])
    [x] = [i for q in analyze.board(R["black-sea"], {src["id"]: "ok"}) for i in q["indicators"]
           if i.get("metric") == "gnss"]
    assert high and (x["status"], x["count"], x["date"], x["sources"]) == \
        ("observed", high, day(2), [src["id"]])
    asked = []
    manifest = "date,suspect,num_bad_aircraft_hexes,source\n" + "".join(
        f"{day(k)},false,1,merged\n" for k in (1, 2, 3, gpsjam.DAYS + 1))
    monkeypatch.setattr(gpsjam, "get", lambda s, **_: asked.append(s["url"]) or
                        SimpleNamespace(text=manifest if s["url"].endswith("manifest.csv")
                                        else "hex,count_good_aircraft,count_bad_aircraft\n"))
    gpsjam.fetch(src)
    assert asked[1:] == [f"{gpsjam.BASE}{day(k)}-h3_4.csv" for k in (1, 2, 3)]
    _, raw, _ = collect.run([s for s in S if s is not src], gpsjam.parse, get=body(d))
    store.append("raw_report", raw)
    asked.clear()
    gpsjam.fetch(src)
    assert asked[1:] == [f"{gpsjam.BASE}{day(k)}-h3_4.csv" for k in (1, 3)]


def test_integrity_filters_follow_gods_eye_view():
    src = next(s for s in integrity.sources(R) if s["region"] == "black-sea")
    assert src["url"] == next(s for s in cycle.sources(["black-sea"], ["adsb"]))["url"]
    base = {"type": "adsb_icao", "version": 2, "alt_baro": 30000, "seen_pos": 0,
            "lat": 44.0, "lon": 34.0, "nic": 8, "nac_p": 9}
    ac = [base | {"hex": "aaa001"},                                   # kept, seen_pos 0
          base | {"hex": "aaa002", "version": 0},                     # no quality fields
          base | {"hex": "aaa003", "alt_baro": "ground"},
          base | {"hex": "aaa004", "seen_pos": 61},
          base | {"hex": "aaa005", "mlat": ["nic", "nac_p"]},         # derived quality only
          base | {"hex": "aaa006", "gpsOkBefore": 1.7e12, "gpsOkLat": 45.0, "gpsOkLon": 35.0},
          base | {"hex": "~aaa007", "lat": 10.0}]                     # outside the box
    [item] = integrity.parse(json.dumps({"now": 1793291463000, "ac": ac}).encode(), src)
    assert item["payload"]["ac"] == [["aaa001", 44.0, 34.0, 8, 9, False],
                                     ["aaa006", 45.0, 35.0, 8, 9, True]]
    assert item["published_at"] == datetime(2026, 10, 29, 16, 31, 3, tzinfo=UTC)


def test_mixed_adapters_on_one_url_parse_by_their_own_adapter():
    S = cycle.sources(["black-sea"], ["adsb", "integrity"], polled=True)
    url = next(s for s in S if s["id"].startswith("integrity:"))["url"]
    G = [s for s in S if s["url"] == url]
    assert {cycle.adapter(s["id"]) for s in G} == {"adsb", "integrity"}
    ac = {"now": 1793291463000, "ac": [{"hex": "43c001", "type": "adsb_icao", "version": 2,
          "alt_baro": 30000, "seen_pos": 1, "lat": 44.0, "lon": 34.0, "nic": 8, "nac_p": 9,
          "dbFlags": 1}]}
    parse = lambda b, s: cycle.COLLECTOR[cycle.adapter(s["id"])].parse(b, s)
    _, raw, runs = collect.run(G, parse, get=body(ac))
    assert sorted(x["source_id"].split(":")[0] for x in raw) == ["adsb", "integrity"]
    assert {x["status"] for x in runs} == {"ok"}


def test_proxy_polls_a_colocated_source_by_its_own_cadence(monkeypatch):
    """The dashboard's aircraft requests must not take integrity snapshots every minute."""
    S = cycle.sources(["black-sea"], ["adsb", "integrity"], polled=True)
    url = next(s for s in S if s["id"].startswith("integrity:"))["url"]
    G = sorted((s for s in S if s["url"] == url), key=lambda s: s["id"])   # adsb first
    calls = []
    monkeypatch.setattr(cycle, "by_url", lambda: {url: G})
    monkeypatch.setattr(cycle, "collect", lambda S, *a, **k: calls.append([s["id"] for s in S]))
    cycle.proxy(url)
    cycle.POLLED[G[1]["region"], G[1]["id"]] = time.monotonic()
    cycle.proxy(url)
    assert calls == [[g["id"] for g in G], [G[0]["id"]]]


def test_own_adsb_cells_and_agreement():
    r = R["black-sea"]
    t = datetime(2026, 10, 29, 12, tzinfo=UTC)
    snap = lambda k, ac: {"id": f"i{k}", "source_id": "integrity:adsb.lol#black-sea",
                          "region": "black-sea", "fetched_at": t + timedelta(minutes=10 * k),
                          "published_at": t + timedelta(minutes=10 * k), "url": None,
                          "payload": json.dumps({"ac": ac})}
    a, b = ["aaa001", 44.0, 34.0, 8, 9, False], ["aaa002", 44.01, 34.01, 8, 9, False]
    c = ["~aaa001", 44.02, 34.02, 8, 5, False]          # a non-ICAO address, not aircraft a
    store.append("raw_report", [snap(0, [a, b, c]), snap(1, [a[:4] + [5, False], b])])
    [cells] = gnss.adsb(r, t - timedelta(days=1)).values()
    assert cells == [[cells[0][0], 1, 2]]                # a bad in one report, c bad, b good
    J = {"2026-10-29": {"cells": [[cells[0][0], 40, 10]]}}
    assert gnss.agreement(J, {"2026-10-29": cells}) == {"both": 1, "gpsjam": 0, "engine": 0,
                                                        "neither": 0}
    J = {"2026-10-29": {"cells": [[cells[0][0], 99, 1]]}}
    assert gnss.agreement(J, {"2026-10-29": cells})["engine"] == 1


# Taiwan MND ------------------------------------------------------------------------

def test_mnd_list_and_report():
    L = mnd.links((FIX / "mnd_list.html").read_text(encoding="utf8"))
    assert L[0] == ("88094", "2026-10-29") and len(L) >= 5
    p = mnd.report("88094", (FIX / "mnd_report_88094.html").read_text(encoding="utf8"))
    assert {k: p[k] for k in ("aircraft", "ships", "official", "balloons", "adiz",
                              "median_line")} == {"aircraft": 6, "ships": 7, "official": 1,
                                                  "balloons": None, "adiz": 5,
                                                  "median_line": None}
    assert (p["start"], p["end"], p["day"]) == ("2026-10-27T22:00:00+00:00",
                                                "2026-10-28T22:00:00+00:00", "2026-10-29")
    assert len(p["text"]) <= collect.EXCERPT


def test_mnd_year_boundary_and_failures():
    jan = datetime(2027, 1, 1, tzinfo=UTC)
    assert mnd.local6("Dec", "31", jan) == datetime(2026, 12, 30, 22, tzinfo=UTC)
    page = '<div class="pageinfo">2026.10.29</div><div class="maincontent">'
    assert mnd.report("1", page + "A statement without a window.</div>") is None
    with pytest.raises(ValueError, match="no counts"):
        mnd.report("1", page + "1.Date: 6 a.m. Oct. 28 to 6 a.m. Oct. 29 (UTC+8) "
                              "2.Nothing to report.</div>")
    with pytest.raises(ValueError, match="parsed"):
        mnd.parse(json.dumps({"1": page + "no window</div>"}).encode(), mnd.sources(R)[0])
    page0 = page + ("1.Date: 6 a.m. Oct. 28 to 6 a.m. Oct. 29 (UTC+8) 2.PLA activities: "
                    "no PLA aircraft, 3 PLAN ships and no official ships.</div>")
    p = mnd.report("2", page0)
    assert (p["aircraft"], p["ships"], p["official"]) == (0, 3, 0)


def test_official_series_median_and_indicator():
    r, src = R["taiwan-strait"], mnd.sources(R)[0]
    t = datetime(2026, 10, 29, 12, tzinfo=UTC)
    raw = [{"id": f"m{k}", "source_id": src["id"], "region": r["id"], "fetched_at": t,
            "published_at": t - timedelta(days=k), "url": None,
            "payload": json.dumps({"id": str(100 - k), "day": (t - timedelta(days=k)).date()
                                   .isoformat(), "aircraft": k, "ships": 5, "official": None,
                                   "balloons": None, "adiz": None, "median_line": None})}
           for k in range(5)]
    store.append("raw_report", raw)
    o = signals.official(r)
    assert o["latest"]["aircraft"] == 0 and o["median"]["aircraft"] == 2.5
    assert o["median"]["official"] is None and o["n_prior"] == 4
    def I(health, t):
        return {i["id"]: i for p in analyze.board(r, health, t) for i in p["indicators"]}
    x = I({src["id"]: "ok"}, t)["TW-1c"]
    assert (x["status"], x["count"], x["date"], x["sources"]) == \
        ("observed", 0, "2026-10-29", [src["id"]])
    assert I({src["id"]: "failed"}, t)["TW-1c"]["status"] == "no data"
    late = t + analyze.RECENT + timedelta(days=1)                 # latest report too old
    assert I({src["id"]: "ok"}, late)["TW-1c"]["status"] == "no data"


def test_an_unstated_official_count_is_no_data():
    """A report that does not state the indicator's count is not a count of zero."""
    r, src = R["taiwan-strait"], mnd.sources(R)[0]
    t = datetime(2026, 10, 29, 12, tzinfo=UTC)
    p = {"id": "1", "day": "2026-10-29", "aircraft": None, "ships": 5, "official": None,
         "balloons": None, "adiz": None, "median_line": None}
    store.append("raw_report", [{"id": "m", "source_id": src["id"], "region": r["id"],
                                 "fetched_at": t, "published_at": t, "url": None,
                                 "payload": json.dumps(p)}])
    B = analyze.board(r, {src["id"]: "ok"}, t)
    [x] = [i for q in B for i in q["indicators"] if i["id"] == "TW-1c"]
    assert (x["status"], x["count"], x["date"]) == ("no data", None, "2026-10-29")
    store.append("indicator_status", analyze.snapshot(r, B, t))
    assert store.query("SELECT count FROM indicator_status WHERE indicator = 'TW-1c'") == \
        [{"count": None}]


# IODA ------------------------------------------------------------------------------

def test_ioda_events_observations_and_linking():
    S = [s for s in ioda.sources(R) if s["code"] == "IR"]
    assert {s["region"] for s in S} == {"iran-israel", "hormuz"} and len({s["url"] for s in S}) == 1
    src = next(s for s in S if s["region"] == "iran-israel")
    items = ioda.parse((FIX / "ioda_events_ir.json").read_bytes(), src)
    assert items and all(i["payload"]["country"] == "IR" for i in items)
    with pytest.raises(ValueError):
        ioda.parse(b'{"error": "bad request", "data": null}', src)
    t0 = 1793000000
    ev = [{"location": "country/IR", "start": t0 + d, "duration": 3600, "datasource": ds,
           "method": "median", "score": 10, "location_name": "Iran"}
          for ds, d in (("bgp", 0), ("ping-slash24", 1800), ("merit-nt", 30000))]
    _, raw, _ = collect.run([src], ioda.parse, get=body({"data": ev, "error": None}))
    O = cycle.derive(raw)["observation"]
    assert {o["kind"] for o in O} == {"internet_outage"} and len(O) == 3
    assert {(o["lat"], o["lon"], o["geo_precision"]) for o in O} == {(35.6892, 51.389, "admin")}
    X = [o | {"outlet": None} for o in O]
    E = fuse.events(R["iran-israel"], X)
    assert sorted(len(e["observation_ids"]) for e in E) == [1, 2]   # within τ link
    assert analyze.gap(analyze.PRODUCERS["internet_outage"], {"ioda:IODA#IR": "ok"}) is False
    assert analyze.gap(analyze.PRODUCERS["internet_outage"], {"rss:BBC": "ok"}) is True
    # Reports of a country, or a region, since removed from the registry derive nothing.
    gone = [raw[0] | {"id": "x1", "payload": json.dumps(json.loads(raw[0]["payload"])
                                                        | {"country": "ZZ"})},
            raw[0] | {"id": "x2", "region": "atlantis", "source_id": "rss:Gone"}]
    assert not any(cycle.derive(gone).values())


# UCDP ------------------------------------------------------------------------------

UCDP = (FIX / "ucdp_candidate_v26_0_9_sample.csv").read_text(encoding="utf8")


def test_ucdp_parse_keeps_region_events_with_version():
    d = {"version": "26.0.9", "url": ucdp.PAGE, "csv": UCDP, "have": []}
    src = next(s for s in ucdp.sources(R) if s["region"] == "south-asia")
    items = ucdp.parse(json.dumps(d).encode(), src)
    assert items and {i["payload"]["country"] for i in items} <= {"Pakistan", "Myanmar (Burma)"}
    assert all(i["native_id"].endswith("@26.0.9") and i["payload"]["version"] == "26.0.9"
               for i in items)
    assert all(len(i["payload"]["where_description"]) <= collect.EXCERPT for i in items)
    # A revised event is a new report; ucdp.events keeps its latest version.
    _, raw, _ = collect.run([src], ucdp.parse, get=body(d))
    later = raw[0] | {"id": "later", "payload": json.dumps(
        json.loads(raw[0]["payload"]) | {"version": "26.0.10", "best": -1})}
    store.append("raw_report", raw + [later])
    U = {u["id"]: u for u in ucdp.events(R["south-asia"])}
    assert len(U) == len(raw) and U[json.loads(later["payload"])["id"]]["best"] == -1


def test_ucdp_version_skipped_only_by_regions_that_store_it(monkeypatch):
    """A poll of one region must not keep the version from the others."""
    S = {s["region"]: s for s in ucdp.sources(R)}
    page = ('<a href="https://ucdp.uu.se/downloads/candidateged/GEDEvent_v26_0_9.csv">'
            '<a href="https://ucdp.uu.se/downloads/candidateged/GEDEvent_v26_0_8.csv">')
    monkeypatch.setattr(ucdp, "get", lambda s, **_: SimpleNamespace(
        text=page, content=UCDP.encode()))
    _, raw, _ = collect.run([S["south-asia"]], ucdp.parse, get=ucdp.fetch)
    store.append("raw_report", raw)
    d = json.loads(ucdp.fetch(S["south-asia"]).content)
    assert (d["version"], d["have"]) == ("26.0.9", ["south-asia"])
    assert ucdp.parse(json.dumps(d).encode(), S["south-asia"]) == []
    other = ucdp.parse(json.dumps(d | {"have": ["hormuz"]}).encode(), S["south-asia"])
    assert len(other) == len(raw) > 0


def test_recall_windows_distance_and_eligibility():
    tz = "UTC"
    u = {"date_start": "2026-10-22", "date_prec": 1, "where_prec": 1, "latitude": 30.0,
         "longitude": 70.0}
    U = [u, u | {"where_prec": 3}, u | {"date_prec": 2}]
    t = datetime(2026, 10, 22, tzinfo=UTC)
    near, far = {"lat": 30.1, "lon": 70.0}, {"lat": 30.5, "lon": 70.0}   # 11 km, 56 km
    period = (t - timedelta(days=3), t + timedelta(days=5))
    assert validate.match(U, [near | {"t_start": t + timedelta(hours=5)}], tz, 0, period)[:2] \
        == (1, 1)
    late = [near | {"t_start": t + timedelta(hours=30)}]
    assert validate.match(U, late, tz, 0, period)[:2] == (1, 0)
    assert validate.match(U, late, tz, 1, period)[:2] == (1, 1)
    assert validate.match(U, [far | {"t_start": t}], tz, 2, period)[:2] == (1, 0)
    assert validate.match(U, [], tz, 0, (t + timedelta(days=1), t + timedelta(days=5)))[0] == 0
    assert round(float(regions.km(30, 70, np.array([30.1]), np.array([70.0]))[0]), 1) == 11.1


# View ------------------------------------------------------------------------------

def test_signals_endpoint_and_board():
    with TestClient(app(collect=False)) as c:
        res = c.get("/api/signals?region=taiwan-strait")
    assert res.status_code == 200
    d = res.json()
    assert set(d) >= {"gnss", "official", "outages", "reference", "sources"}
    assert d["official"]["latest"] is None and d["reference"] is None
    B = analyze.board(R["taiwan-strait"], {})
    I = {i["id"]: i for p in B for i in p["indicators"]}
    assert I["TW-1c"]["status"] == I["TW-3a"]["status"] == "no data"
    assert I["TW-3b"]["status"] == "no data"                    # no healthy IODA source
