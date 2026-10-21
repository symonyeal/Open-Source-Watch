"""Certificates for fusion (spec § Fusion) and credibility (spec § Credibility)."""
from datetime import UTC, datetime, timedelta

import numpy as np
from hypothesis import given, settings
from hypothesis import strategies as st

from engine import analyze, fuse, regions, store

R = regions.regions()
T0 = datetime(2026, 10, 21, tzinfo=UTC)


def brute(t, lat, lon, fine, country):
    """Connected components of the link graph by exhaustive pair check and BFS."""
    n, adj = len(t), [[] for _ in t]
    for i in range(n):
        for j in range(i + 1, n):
            if abs(t[i] - t[j]) > fuse.TAU:
                continue
            if fine[i] and fine[j]:
                ok = fuse.haversine(lat[i], lon[i], lat[j], lon[j]) <= fuse.DELTA
            else:
                ok = lat[i] == lat[j] and lon[i] == lon[j] and not (country[i] or country[j])
            if ok:
                adj[i].append(j)
                adj[j].append(i)
    lab, seen = [-1] * n, set()
    for s in range(n):
        if s in seen:
            continue
        stack, C = [s], []
        while stack:
            u = stack.pop()
            if u not in seen:
                seen.add(u)
                C.append(u)
                stack += adj[u]
        for u in C:
            lab[u] = min(C)
    return lab


def partition(lab):
    G = {}
    for i, c in enumerate(lab):
        G.setdefault(int(c), set()).add(i)
    return {frozenset(g) for g in G.values()}


obs = st.lists(st.tuples(st.floats(0, 6 * 3600), st.sampled_from([(48.0, 35.0), (48.1, 35.1),
                                                                  (48.3, 35.0), (49.0, 32.0)]),
                         st.sampled_from(["point", "city", "admin", "country"])),
               min_size=1, max_size=40)


@settings(max_examples=200, deadline=None)
@given(obs)
def test_components_equal_brute_force(X):
    X = sorted(X)
    t = np.array([x[0] for x in X])
    lat, lon = np.array([x[1][0] for x in X]), np.array([x[1][1] for x in X])
    fine = np.array([x[2] in fuse.FINE for x in X])
    country = np.array([x[2] == "country" for x in X])
    assert partition(fuse.components(t, lat, lon, fine, country)) == \
        partition(brute(t, lat, lon, fine, country))


def observation(k, minutes, lat, lon, src, kind="explosions_remote_violence", prec="city"):
    return {"id": f"o{k}", "raw_id": f"r{k}", "source_id": src, "region": "iran-israel",
            "t": T0 + timedelta(minutes=minutes), "lat": lat, "lon": lon,
            "geo_precision": prec, "kind": kind, "text_en": "", "lang": "en",
            "credibility": 3, "names": [], "outlet": None}


@settings(max_examples=100, deadline=None)
@given(st.permutations(range(6)))
def test_events_do_not_depend_on_input_order(p):
    X = [observation(0, 0, 32.0, 35.0, "rss:BBC"), observation(1, 30, 32.1, 35.1, "rss:CNN"),
         observation(2, 200, 32.0, 35.0, "rss:WSJ"), observation(3, 10, 30.0, 50.0, "rss:NYT"),
         observation(4, 5, 32.5, 53.0, "rss:BBC", prec="country"),
         observation(5, 6, 32.5, 53.0, "rss:CNN", prec="country")]
    E = fuse.events(R["iran-israel"], [X[i] for i in p])
    assert {frozenset(e["observation_ids"]) for e in E} == \
        {frozenset({"o0", "o1"}), frozenset({"o2"}), frozenset({"o3"}), frozenset({"o4"}),
         frozenset({"o5"})}


def test_reports_and_unlocated_observations_form_no_event():
    X = [observation(0, 0, 32.0, 35.0, "rss:BBC", kind="report"),
         observation(1, 0, None, None, "rss:BBC", prec="none")]
    assert fuse.events(R["iran-israel"], X) == []


grades = st.lists(st.tuples(st.sampled_from("ABCDEF"), st.sampled_from("uvwxyz")), max_size=8)


@given(grades, st.sampled_from("uvwxyz"))
def test_credibility_is_monotone_in_corroboration(S, o):
    """Adding an A/B source never makes an event less credible (a larger number)."""
    assert fuse.credibility(S + [("A", o)]) <= fuse.credibility(S)


@given(grades)
def test_contradiction_forces_improbable(S):
    assert fuse.credibility(S, contradicted=True) == 5


def test_credibility_table():
    assert [fuse.credibility(S) for S in (
        [("A", "x"), ("B", "y"), ("B", "z")],     # n = 3
        [("A", "x"), ("B", "y")],                 # n = 2
        [("B", "x"), ("B", "x")],                 # one owner: n = 1
        [("C", "x"), ("E", "y")],
        [("F", "x")], [])] == [1, 2, 3, 4, 6, 6]


def test_google_news_independence_is_by_outlet():
    r = R["iran-israel"]
    a = observation(0, 0, 32.0, 35.0, "gnews:strike#1") | {"outlet": "Reuters"}
    b = observation(1, 0, 32.0, 35.0, "gnews:strike#2") | {"outlet": "AP"}
    assert fuse.owner(r, a) != fuse.owner(r, b)
    assert fuse.owner(r, observation(2, 0, 0, 0, "rss:BBC")) == "BBC"


def test_indicator_board():
    r = R["iran-israel"]
    now = datetime.now(UTC)
    E = [{"id": f"e{k}", "region": r["id"], "t_start": now - timedelta(hours=1),
          "t_end": now - timedelta(hours=1), "lat": 32.0, "lon": 35.0,
          "kind": "air_raid_alert", "observation_ids": [f"o{k}"], "n_independent": 1,
          "credibility": 3} for k in range(10)]
    store.replace("event", E)
    store.append("observation", [observation(k, 0, 32.0, 35.0, "alerts:tzevaadom",
                                             kind="air_raid_alert") | {"t": now}
                                 for k in range(2)])
    B = {p["pir"]: p for p in analyze.board(r, {"alerts:tzevaadom": "ok"})}
    i = B["II-1"]["indicators"][0]
    assert (i["count"], i["status"], B["II-1"]["status"]) == (10, "triggered", "answered")
    assert B["II-2"]["status"] == "gap"           # no healthy text source can report them
    B = {p["pir"]: p for p in analyze.board(r, {"rss:BBC": "ok"})}
    assert B["II-2"]["status"] == "open"          # healthy, and nothing happened: no gap
    B = {p["pir"]: p for p in analyze.board(r, {"alerts:tzevaadom": "failed"})}
    assert B["II-1"]["status"] == "answered"      # triggered takes precedence over a gap
    assert analyze.status(0, 1, 3) == "inactive" and analyze.status(1, 1, 3) == "watch"
