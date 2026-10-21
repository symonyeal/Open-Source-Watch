"""Region registry (spec § Seam): regions/<id>/region.toml, engine-owned, joined
with regions/<id>/conflict.json, exported from the TypeScript conflict config.

A source id is "<adapter>:<name>" or "<adapter>:<name>#<k>" when several feeds
share a name; grades and owners are keyed by the part before '#'.
"""
import json
import re
import tomllib
from functools import cache
from pathlib import Path

DIR = Path(__file__).resolve().parent.parent / "regions"
GRADES = "ABCDEF"


@cache
def regions():
    """{id: region dict}; region["conflict"] is the exported TypeScript config."""
    R = {}
    for p in sorted(DIR.glob("*/region.toml")):
        r = tomllib.loads(p.read_text(encoding="utf8"))
        r["conflict"] = json.loads((p.parent / "conflict.json").read_text(encoding="utf8"))
        r.setdefault("owner", {})
        r.setdefault("pir", [])
        R[r["id"]] = r
    return R


def key(source_id):
    return source_id.split("#")[0]


def reliability(r, source_id):
    """Admiralty grade A-F; F (cannot be judged) for an ungraded source."""
    return r["reliability"].get(key(source_id), "F")


def owner(r, source_id):
    k = key(source_id)
    return r["owner"].get(k, k.split(":", 1)[1])


def regex(d):
    """JavaScript RegExp {regex, flags} as compiled Python pattern."""
    return re.compile(d["regex"], re.IGNORECASE if "i" in d["flags"] else 0)


def in_bbox(r, lat, lon):
    a, b, c, d = r["bbox"]
    return a <= lat <= b and c <= lon <= d
