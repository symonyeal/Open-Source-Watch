from datetime import UTC, datetime

import httpx
import pytest

from engine import collect
from engine.collect import rss
from tests.conftest import FIX


def items(name, src):
    return rss.parse((FIX / name).read_bytes(), src)


def test_rss_fixture():
    X = items("bbc_rss.xml", "BBC")
    assert [x["payload"]["title"] for x in X][2] == \
        "Houthi missile strike on Yemen camp kills 73 security recruits, local authorities say"
    assert X[0]["published_at"] == datetime(2026, 10, 20, 8, 53, 6, tzinfo=UTC)


def test_google_news_outlet_suffix_is_stripped():
    X = items("google_news.xml", "Google News")
    assert X[0]["payload"]["title"] == "Benjamin Netanyahu pushes to resume strikes against Iran"


def test_atom_rfc4287_example():
    """RFC 4287 §1.1: the minimal single-entry Atom document."""
    [x] = items("rfc4287_example.xml", "X")
    assert x["payload"]["title"] == "Atom-Powered Robots Run Amok"
    assert x["url"] == "http://example.org/2003/12/13/atom03"
    assert x["published_at"] == datetime(2003, 12, 13, 18, 30, 2, tzinfo=UTC)


def test_excerpt_bound():
    X = items("meduza.xml", "Meduza")
    assert all(len(x["payload"]["title"]) <= collect.EXCERPT for x in X)


def _raise(e):
    def f():
        raise e
    return f


REQ = httpx.Request("GET", "http://x")


@pytest.mark.parametrize("fetch, status", [
    (lambda: items("bbc_rss.xml", "BBC"), "ok"),
    (list, "empty"),
    (_raise(httpx.HTTPStatusError("", request=REQ, response=httpx.Response(429))), "blocked"),
    (_raise(httpx.HTTPStatusError("", request=REQ, response=httpx.Response(500))), "http_error"),
    (_raise(httpx.ConnectTimeout("")), "http_error"),
    (lambda: rss.parse(b"<!DOCTYPE html><html>", "X"), "parse_error"),
])
def test_failure_is_a_status_not_an_exception(fetch, status):
    raw, row = collect.run("iran-israel", "rss:BBC", fetch)
    assert row["status"] == status and row["n_rows"] == len(raw)


def test_raw_ids_are_content_hashes():
    f = lambda: items("bbc_rss.xml", "BBC")
    a, _ = collect.run("iran-israel", "rss:BBC", f)
    b, _ = collect.run("iran-israel", "rss:BBC", f)
    assert [x["id"] for x in a] == [x["id"] for x in b]
    assert len({x["id"] for x in a}) == len(a)
