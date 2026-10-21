import shutil
import subprocess

import pytest

from engine import regions
from engine.collect import rss
from engine.schema import KINDS

R = regions.regions()


@pytest.mark.skipif(not shutil.which("node"), reason="node not installed")
def test_conflict_json_is_a_fresh_export():
    """regions/*/conflict.json equals the export of the TypeScript configs."""
    p = subprocess.run(["node", "--no-warnings", "scripts/export-regions.mjs", "--check"],
                       capture_output=True, text=True, check=False)
    assert p.returncode == 0, p.stderr


@pytest.mark.parametrize("r", R.values(), ids=list(R))
def test_every_source_is_graded(r):
    S = [f"rss:{f['name']}" for f in r["conflict"]["newsFeeds"]] + \
        [f"telegram:{c['name']}" for c in r["conflict"]["telegramChannels"]]
    assert [s for s in S if s not in r["reliability"]] == []
    assert set(r["reliability"].values()) <= set(regions.GRADES)
    assert set(r["owner"]) <= set(r["reliability"])


@pytest.mark.parametrize("r", R.values(), ids=list(R))
def test_region_geometry_and_rules(r):
    a, b, c, d = r["bbox"]
    assert -90 <= a < b <= 90 and -180 <= c < d <= 180
    bb = r["conflict"]["firesBBox"]
    assert [bb["latMin"], bb["latMax"], bb["lonMin"], bb["lonMax"]] == r["bbox"]
    regions.regex(r["conflict"]["newsRelevanceKeywords"])
    assert len(rss.feeds(r)) == len(r["conflict"]["newsFeeds"])


@pytest.mark.parametrize("r", R.values(), ids=list(R))
def test_pir_indicators(r):
    I = [i for p in r["pir"] for i in p["indicator"]]
    assert I and all(i["kind"] in KINDS and 1 <= i["watch"] <= i["trigger"] for i in I)
    assert len({i["id"] for i in I}) == len(I)
