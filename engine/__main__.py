"""py -3.14 -m engine collect [--region R ...] [--adapter A ...] [--runner local|cron]
py -3.14 -m engine serve [--port 8790] [--no-collect]
py -3.14 -m engine rebuild"""
import argparse
from collections import Counter

from engine import cycle

p = argparse.ArgumentParser(prog="engine")
sub = p.add_subparsers(dest="cmd", required=True)
c = sub.add_parser("collect")
c.add_argument("--region", nargs="*")
c.add_argument("--adapter", nargs="*")
c.add_argument("--runner", default="local")
s = sub.add_parser("serve")
s.add_argument("--port", type=int, default=8790)
s.add_argument("--no-collect", action="store_true")
sub.add_parser("rebuild")
a = p.parse_args()

if a.cmd == "collect":
    runs = cycle.collect(cycle.tasks(a.region, a.adapter), a.runner)
    print(dict(Counter(x["status"] for x in runs)), sum(x["n_rows"] for x in runs), "items")
elif a.cmd == "serve":
    import uvicorn

    from engine.api import app
    uvicorn.run(app(collect=not a.no_collect), host="127.0.0.1", port=a.port)
else:
    print(cycle.rebuild(), "raw reports reprocessed")
