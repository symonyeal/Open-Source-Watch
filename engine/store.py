"""Append-only Parquet store, read with DuckDB (spec § Store).

Files live at <data>/<table>/<region>/<date>/<uuid>.parquet. Rows with an `id`
are deduplicated on read (first `fetched_at`/`t` wins), so re-collecting the
same item is idempotent (spec § Errors). The data root is $OSW_DATA or ./data.
"""
import os
import shutil
import uuid
from collections import defaultdict
from pathlib import Path

import duckdb
import pyarrow as pa
import pyarrow.parquet as pq

from engine.schema import SCHEMA

ROOT = Path(__file__).resolve().parent.parent


def data_dir():
    return Path(os.environ.get("OSW_DATA", ROOT / "data"))


def _day(row):
    t = row.get("fetched_at") or row.get("started_at") or row.get("t") or row.get("t_start")
    return t.date().isoformat() if t else "undated"


def append(table, rows):
    """Write rows (dicts) to `table`, one file per (region, date). Returns len(rows)."""
    parts = defaultdict(list)
    for r in rows:
        parts[(r.get("region") or "global", _day(r))].append(r)
    for (region, day), rs in parts.items():
        d = data_dir() / table / region / day
        d.mkdir(parents=True, exist_ok=True)
        pq.write_table(pa.Table.from_pylist(rs, SCHEMA[table]), d / f"{uuid.uuid4().hex}.parquet")
    return len(rows)


def drop(table):
    shutil.rmtree(data_dir() / table, ignore_errors=True)


ORDER = {"raw_report": "fetched_at", "observation": "t", "event": "t_start", "entity": "id"}


def connect():
    """DuckDB connection with one view per table (deduplicated by id where defined)."""
    con = duckdb.connect()
    con.execute("SET TimeZone = 'UTC'")
    for t, schema in SCHEMA.items():
        files = sorted((data_dir() / t).glob("*/*/*.parquet"))
        if not files:
            con.register(f"{t}_empty", pa.Table.from_pylist([], schema))
            con.execute(f"CREATE VIEW {t} AS SELECT * FROM {t}_empty")
            continue
        src = f"read_parquet({[str(f) for f in files]!r}, union_by_name = true)"
        if t in ORDER:
            src = (f"(SELECT * FROM {src} QUALIFY row_number() "
                   f"OVER (PARTITION BY id ORDER BY {ORDER[t]}) = 1)")
        con.execute(f"CREATE VIEW {t} AS SELECT * FROM {src}")
    return con


def query(sql, params=()):
    """Rows of `sql` as dicts."""
    con = connect()
    cur = con.execute(sql, params)
    cols = [c[0] for c in cur.description]
    return [dict(zip(cols, r)) for r in cur.fetchall()]
