"""Append-only Parquet store, read with DuckDB (spec § Store).

Files live at <data>/<table>/<region>/<date>/<uuid>.parquet. Rows with an `id`
are deduplicated on read (first `fetched_at`/`t` wins), so re-collecting the
same item is idempotent (spec § Errors). The data root is $OSW_DATA or ./data.
"""
import os
import shutil
import threading
import uuid
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path

import duckdb
import pyarrow as pa
import pyarrow.parquet as pq

from engine.schema import SCHEMA

ROOT = Path(__file__).resolve().parent.parent
LOCK = threading.RLock()   # compaction deletes files; queries must not see that happen


def data_dir():
    return Path(os.environ.get("OSW_DATA", ROOT / "data"))


def _day(row):
    t = row.get("fetched_at") or row.get("started_at") or row.get("t") or row.get("t_start")
    return t.date().isoformat() if t else "undated"


def _write(root, table, rows):
    """One file per (region, date) of rows under root/<region>/<date>/."""
    parts = defaultdict(list)
    for r in rows:
        parts[(r.get("region") or "global", _day(r))].append(r)
    for (region, day), rs in parts.items():
        d = root / region / day
        d.mkdir(parents=True, exist_ok=True)
        pq.write_table(pa.Table.from_pylist(rs, SCHEMA[table]), d / f"{uuid.uuid4().hex}.parquet")
    return len(rows)


def append(table, rows):
    """Write rows (dicts) to `table`. Returns len(rows)."""
    return _write(data_dir() / table, table, rows)


def drop(table):
    with LOCK:
        shutil.rmtree(data_dir() / table, ignore_errors=True)


def replace(table, rows):
    """Swap `table` for exactly `rows` (a recomputed derived table), atomically
    with respect to queries."""
    tmp = data_dir() / f".{table}.new"
    shutil.rmtree(tmp, ignore_errors=True)
    n = _write(tmp, table, rows)
    with LOCK:
        shutil.rmtree(data_dir() / table, ignore_errors=True)
        if tmp.exists():
            tmp.rename(data_dir() / table)
    return n


def compact(before=None):
    """Merge the files of each <table>/<region>/<date> directory dated before
    `before` (default: today, UTC) into one file. Returns the files removed."""
    before = before or datetime.now(UTC).date().isoformat()
    n = 0
    with LOCK:
        for d in data_dir().glob("*/*/*"):
            F = sorted(d.glob("*.parquet"))
            if len(F) < 2 or d.name >= before or d.parts[-3] not in SCHEMA:
                continue
            T = pa.concat_tables([pq.read_table(f) for f in F], promote_options="default")
            pq.write_table(T.cast(SCHEMA[d.parts[-3]]), d / f"{uuid.uuid4().hex}.parquet")
            for f in F:
                f.unlink()
            n += len(F)
    return n


ORDER = {"raw_report": "fetched_at", "observation": "t", "event": "t_start", "entity": "id",
         "product": "t"}


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
    with LOCK:
        cur = connect().execute(sql, params)
        cols = [c[0] for c in cur.description]
        return [dict(zip(cols, r)) for r in cur.fetchall()]
