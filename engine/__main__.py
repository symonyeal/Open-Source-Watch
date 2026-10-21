"""py -3.14 -m engine collect [--region R ...] [--adapter A ...] [--runner local|cron] [--due]
py -3.14 -m engine serve [--port 8790] [--no-collect]
py -3.14 -m engine rebuild
py -3.14 -m engine compact
py -3.14 -m engine fuse
py -3.14 -m engine regions   # write src/lib/conflicts/generated.ts
py -3.14 -m engine gazetteer cities15000.zip"""
import argparse
from collections import Counter

from engine import cycle, store

p = argparse.ArgumentParser(prog="engine")
sub = p.add_subparsers(dest="cmd", required=True)
c = sub.add_parser("collect")
c.add_argument("--region", nargs="*")
c.add_argument("--adapter", nargs="*")
c.add_argument("--runner", default="local")
c.add_argument("--due", action="store_true", help="only sources whose cadence has elapsed")
s = sub.add_parser("serve")
s.add_argument("--port", type=int, default=8790)
s.add_argument("--no-collect", action="store_true")
sub.add_parser("rebuild")
sub.add_parser("compact")
sub.add_parser("fuse")
sub.add_parser("regions")
sub.add_parser("gazetteer").add_argument("zip")
a = p.parse_args()

if a.cmd == "collect":
    S = cycle.sources(a.region, a.adapter, polled=True)
    runs = cycle.collect(cycle.due(S) if a.due else S, a.runner)
    print(dict(Counter(x["status"] for x in runs)), sum(x["n_rows"] for x in runs), "items")
elif a.cmd == "serve":
    import uvicorn

    from engine.api import app
    uvicorn.run(app(collect=not a.no_collect), host="127.0.0.1", port=a.port)
elif a.cmd == "rebuild":
    print(cycle.rebuild(), "raw reports reprocessed")
elif a.cmd == "compact":
    print(store.compact(), "files merged")
elif a.cmd == "regions":
    from engine import regions
    regions.TS.write_text(regions.build_ts(), encoding="utf8")
    print(sum("config" in r for r in regions.regions().values()), "TOML regions written")
elif a.cmd == "fuse":
    from engine import fuse
    print(fuse.fuse(), "events")
else:
    from engine.process import build_geonames
    print(build_geonames(a.zip), "gazetteer names written")
