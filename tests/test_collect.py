import json
import shutil
import subprocess
from datetime import UTC, datetime
from types import SimpleNamespace

import httpx
import pytest

from engine import collect, cycle, regions
from engine.collect import adsb, alerts, firms, markets, neptun, rss, telegram
from tests.conftest import FIX

R = regions.regions()


def body(name):
    return (FIX / name).read_bytes()


def items(name, src_name):
    return rss.parse(body(name), {"name": src_name})


def test_rss_fixture():
    X = items("bbc_rss.xml", "BBC")
    assert X[2]["payload"]["title"] == \
        "Houthi missile strike on Yemen camp kills 73 security recruits, local authorities say"
    assert X[0]["published_at"] == datetime(2026, 10, 20, 8, 53, 6, tzinfo=UTC)


def test_google_news_outlet_suffix_is_split():
    p = items("google_news.xml", "Google News")[0]["payload"]
    assert (p["title"], p["outlet"]) == \
        ("Benjamin Netanyahu pushes to resume strikes against Iran", "Financial Times")


def test_atom_rfc4287_example():
    """RFC 4287 §1.1: the minimal single-entry Atom document."""
    [x] = items("rfc4287_example.xml", "X")
    assert x["payload"]["title"] == "Atom-Powered Robots Run Amok"
    assert x["url"] == "http://example.org/2003/12/13/atom03"
    assert x["published_at"] == datetime(2003, 12, 13, 18, 30, 2, tzinfo=UTC)


def test_excerpt_bound():
    X = items("meduza.xml", "Meduza") + telegram.parse(body("telegram_DeepStateUA.html"), {})
    assert all(len(x["payload"].get("title") or x["payload"]["text"]) <= collect.EXCERPT
               for x in X)


def test_telegram_preview_page():
    X = telegram.parse(body("telegram_DeepStateUA.html"), {})
    assert [x["native_id"] for x in X] == [23911, 23912, 23913]
    assert X[0]["url"] == "https://t.me/DeepStateUA/23911"
    assert X[0]["published_at"] == datetime(2026, 10, 19, 11, 0, 43, tzinfo=UTC)
    assert telegram.parse(b"<html><div class='tgme_page'></div></html>", {}) == []


def test_firms_keeps_detections_inside_some_region():
    X = firms.parse(body("firms.csv"), {})
    assert len(X) == 7      # 4 Middle East + 3 Eastern Europe; 2 southern-hemisphere dropped
    assert all(any(regions.in_bbox(r, float(x["payload"]["latitude"]),
                                   float(x["payload"]["longitude"])) for r in R.values())
               for x in X)


def test_adsb_military_feed_is_clipped_to_regions():
    X = adsb.parse(body("adsb_mil.json"), {"region": "global"})
    assert len(X) == 3 and len({x["native_id"] for x in X}) == 3


def test_alerts_providers():
    X = alerts.parse(body("alertsua_states.json"), {"provider": "alertsua"})
    assert [x["payload"]["locations"] for x in X] == [["Dnipropetrovsk oblast"]]
    [y] = alerts.parse(body("tzevaadom_synthetic.json"), {"provider": "tzevaadom"})
    assert y["payload"]["locations"] == ["שדרות", "אשקלון"]
    assert alerts.parse(b"[]", {"provider": "tzevaadom"}) == []


def test_neptun_tracks():
    X = neptun.parse(body("neptun.json"), {})
    assert len(X) == 4 and all(isinstance(x["payload"]["lat"], float) for x in X)  # markers


def test_markets():
    [y] = markets.parse(body("yahoo_LMT.json"), {"kind": "yahoo"})
    assert y["payload"]["symbol"] == "LMT"
    assert len(markets.parse(body("coingecko.json"), {"kind": "coingecko"})) == 4


@pytest.mark.skipif(not shutil.which("node"), reason="node not installed")
def test_polymarket_filter_equals_javascript():
    """A market is kept for a region iff upstream's rule, evaluated by JavaScript,
    keeps it: keywords match and exclude does not (src/app/api/polymarket)."""
    js = ("const fs=require('fs');const Q=JSON.parse(fs.readFileSync('tests/fixtures/"
          "polymarket.json','utf8')).map(m=>m.question);const out={};for(const r of "
          "['iran-israel','russia-ukraine']){const c=JSON.parse(fs.readFileSync(`regions/${r}/"
          "conflict.json`,'utf8'));const K=new RegExp(c.polymarketKeywords.regex,"
          "c.polymarketKeywords.flags),X=new RegExp(c.polymarketExclude.regex,"
          "c.polymarketExclude.flags);out[r]=Q.filter(q=>K.test(q)&&!X.test(q))}"
          "console.log(JSON.stringify(out))")
    want = json.loads(subprocess.run(["node", "-e", js], capture_output=True, text=True,
                                     check=True).stdout)
    P = markets.parse(body("polymarket.json"), {"kind": "polymarket"})
    for r, Q in want.items():
        assert [p["payload"]["question"] for p in P if r in p["payload"]["regions"]] == Q


@pytest.mark.skipif(not shutil.which("node"), reason="node not installed")
def test_encode_uri_component_equals_javascript():
    S = ["Iran Israel war", "Ukraine \"front line\" OR offensive", "a/b?c=d&e", "שדרות", "~*'()!"]
    code = f"console.log(JSON.stringify({json.dumps(S)}.map(encodeURIComponent)))"
    js = subprocess.run(["node", "-e", code], capture_output=True, text=True, check=True).stdout
    assert [rss.encode_uri_component(s) for s in S] == json.loads(js)


def test_urls_are_built_as_upstream_builds_them():
    S = {s["id"]: s["url"] for s in cycle.sources(["iran-israel"])}
    c = R["iran-israel"]["conflict"]
    assert S["gnews:strike#1"] == rss.GNEWS.format(c["strikeQueries"][0])
    assert S["gnews:conflict#1"] == rss.GNEWS.format(rss.encode_uri_component(
        c["conflictQueries"][0]))
    f = c["flightsCenter"]
    assert S["adsb:adsb.lol#iran-israel"] == \
        f"https://api.adsb.lol/v2/lat/{f['lat']}/lon/{f['lon']}/dist/{f['dist']}"
    M = {s["id"]: s["url"] for s in cycle.sources(["global"])}
    assert M["markets:Yahoo Finance#^GSPC"].endswith("chart/%5EGSPC?interval=1d&range=5d")
    assert M["markets:Yahoo Finance#CL=F"].endswith("chart/CL=F?interval=1d&range=5d")


def ok(name):
    return lambda src: SimpleNamespace(content=body(name), status_code=200, headers={})


def fail(e):
    def get(src):
        raise e
    return get


REQ = httpx.Request("GET", "http://x")
SRC = [{"id": "rss:BBC", "region": "iran-israel", "name": "BBC"}]


@pytest.mark.parametrize("get, status", [
    (ok("bbc_rss.xml"), "ok"),
    (lambda src: SimpleNamespace(content=b"<rss><channel/></rss>"), "empty"),
    (fail(httpx.HTTPStatusError("", request=REQ, response=httpx.Response(429))), "blocked"),
    (fail(httpx.HTTPStatusError("", request=REQ, response=httpx.Response(500))), "http_error"),
    (fail(httpx.ConnectTimeout("")), "http_error"),
    (lambda src: SimpleNamespace(content=b"<!DOCTYPE html><html>"), "parse_error"),
])
def test_failure_is_a_status_not_an_exception(get, status):
    _, raw, [row] = collect.run(SRC, rss.parse, get=get)
    assert row["status"] == status and row["n_rows"] == len(raw)


def test_one_fetch_serves_every_source_on_the_url():
    S = [dict(SRC[0]), {"id": "rss:BBC", "region": "russia-ukraine", "name": "BBC"}]
    _, raw, runs = collect.run(S, rss.parse, get=ok("bbc_rss.xml"))
    assert [r["n_rows"] for r in runs] == [3, 3] and len({x["id"] for x in raw}) == 3


def test_raw_ids_are_content_hashes():
    a = collect.run(SRC, rss.parse, get=ok("bbc_rss.xml"))[1]
    b = collect.run(SRC, rss.parse, get=ok("bbc_rss.xml"))[1]
    assert [x["id"] for x in a] == [x["id"] for x in b]
