"""gpsjam.org daily navigation-accuracy cells (docs/SOURCES.md).

gpsjam.org publishes, for each UTC day, every H3 resolution-4 cell that aircraft
flew through with the number of aircraft that reported good and bad navigation
accuracy (`<day>-h3_4.csv`), and a manifest of the days with a `suspect` flag
for incomplete ones. One raw report is one region's cells of one day, the cells
whose centre lies in the region's box; its native id is the day, so a day is
stored once. Each poll fetches the manifest and the days of the last DAYS that
some region does not store (all of them on the first poll).
"""
import csv
import io
import json
from collections import defaultdict
from datetime import UTC, date, datetime, timedelta
from functools import lru_cache

import numpy as np

from engine import regions
from engine.collect import fetch as get
from engine.collect import response, stored

BASE = "https://gpsjam.org/data/"
DAYS = 30
CADENCE = 43200


def sources(R):
    return [{"id": f"gpsjam:gpsjam.org#{rid}", "region": rid, "url": BASE + "manifest.csv",
             "cadence": CADENCE} for rid in R]


def fetch(src):
    """The manifest and each day of the last DAYS some region lacks, as one JSON
    body {"manifest": {day: row}, "days": {day: csv text}}. Every region stores a
    report for every day it parses, so a day is complete when all regions have it."""
    M = {r["date"]: r for r in csv.DictReader(io.StringIO(get(src).text))}
    t0 = datetime.now(UTC).date() - timedelta(days=DAYS)
    have = defaultdict(set)
    for x in stored("gpsjam", since=t0):
        have[x["payload"]["date"]].add(x["region"])
    want = [d for d in M if date.fromisoformat(d) >= t0 and not regions.regions().keys() <= have[d]]
    days = {d: get({"url": f"{BASE}{d}-h3_4.csv"}).text for d in want}
    return response({"manifest": {d: M[d] for d in want}, "days": days})


@lru_cache(maxsize=64)
def table(text):
    """(hex strings, good, bad, centre lat, centre lon) of one day's CSV; cached
    because every region parses the same day."""
    import osw_kernels
    R = list(csv.DictReader(io.StringIO(text)))
    h = np.array([int(r["hex"], 16) for r in R], np.uint64)
    lat, lon = osw_kernels.centres(h) if len(h) else (np.empty(0), np.empty(0))
    return ([r["hex"] for r in R], [int(r["count_good_aircraft"]) for r in R],
            [int(r["count_bad_aircraft"]) for r in R], lat, lon)


def parse(body, src):
    d, r = json.loads(body), regions.regions()[src["region"]]
    items = []
    for day, text in d["days"].items():
        H, G, B, lat, lon = table(text)
        C = [[h, g, b] for h, g, b, a, o in zip(H, G, B, lat, lon) if regions.in_bbox(r, a, o)]
        m = d["manifest"][day]
        items.append({"native_id": day, "url": f"{BASE}{day}-h3_4.csv",
                      "published_at": datetime.fromisoformat(day).replace(tzinfo=UTC),
                      "payload": {"date": day, "suspect": m.get("suspect") == "true",
                                  "network": m.get("source"), "res": 4, "cells": C}})
    return items
