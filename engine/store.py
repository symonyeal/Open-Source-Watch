"""Append-only Parquet store, read with DuckDB (spec § Store).

Files live at <data>/<table>/<region>/<UTC day written>/<uuid>.parquet. Rows with an `id`
are deduplicated on read (first `fetched_at`/`t` wins, ties broken by the row's
content, so every read picks the same copy), so re-collecting the
same item is idempotent (spec § Errors). The data root is $OSW_DATA or ./data.
"""
import os
import re
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


def _write(root, table, rows):
    """One file per region of rows under root/<region>/<UTC day written>/. The day is
    the write date, not a row time: observations carry publication times years apart,
    and one directory per such date left the store as ~1,000 small files that every
    query scans."""
    day = datetime.now(UTC).date().isoformat()
    parts = defaultdict(list)
    for r in rows:
        parts[r.get("region") or "global"].append(r)
    for region, rs in parts.items():
        d = root / region / day
        d.mkdir(parents=True, exist_ok=True)
        path = d / f"{uuid.uuid4().hex}.pending"
        pq.write_table(pa.Table.from_pylist(rs, SCHEMA[table]), path)
        path.rename(path.with_suffix(".parquet"))
    return len(rows)


def append(table, rows):
    """Write rows (dicts) to `table`. Returns len(rows)."""
    with LOCK:
        return _write(data_dir() / table, table, rows)


def _discard(table):
    """Remove the table's directory. It is renamed aside first: on Windows a deleted
    directory keeps its name while any handle (an indexer, antivirus, file watcher)
    is open on it, and a new directory cannot take that name until then."""
    d = data_dir() / table
    if d.exists():
        d.rename(data_dir() / f".{table}.{uuid.uuid4().hex}.old")
    for old in data_dir().glob(f".{table}.*.old"):
        shutil.rmtree(old, ignore_errors=True)


def drop(table):
    with LOCK:
        _discard(table)


def replace(table, rows):
    """Swap `table` for exactly `rows` (a recomputed derived table), atomically
    with respect to queries."""
    tmp = data_dir() / f".{table}.{uuid.uuid4().hex}.new"
    n = _write(tmp, table, rows)
    with LOCK:
        _discard(table)
        if tmp.exists():
            tmp.rename(data_dir() / table)
    return n


def compact(before=None):
    """Merge each <table>/<region>/<day> directory dated before `before` (default:
    today, UTC) into one file, and every directory of a month before `before`'s month
    into one file under <table>/<region>/<YYYY-MM>. A row is rewritten at most twice,
    so a store kept under git holds at most three copies of its data, and the file
    count grows by month, not by write. Returns the files removed."""
    before = before or datetime.now(UTC).date().isoformat()
    n = 0
    with LOCK:
        for r in data_dir().glob("*/*"):
            if r.parent.name not in SCHEMA or not r.is_dir():
                continue
            G = defaultdict(list)
            for d in r.iterdir():
                if d.name < before:
                    G[d.name[:7] if d.name[:7] < before[:7] else d.name].append(d)
            for key, D in G.items():
                F = sorted(f for d in D for f in d.glob("*.parquet"))
                if len(F) < 2:
                    continue
                T = pa.concat_tables([pq.read_table(f) for f in F], promote_options="default")
                out = r / key
                out.mkdir(exist_ok=True)
                pq.write_table(T.cast(SCHEMA[r.parent.name]), out / f"{uuid.uuid4().hex}.parquet")
                for f in F:
                    f.unlink()
                for d in D:
                    if d.name != key and not any(d.iterdir()):
                        d.rmdir()
                n += len(F)
    return n


ORDER = {"raw_report": "fetched_at", "observation": "t", "event": "t_start", "entity": "id",
         "product": "t"}


def connect(tables=SCHEMA):
    """DuckDB connection with one view per table (deduplicated by id where defined)
    and, for every table t, a view t_rows of each stored row, duplicates included."""
    con = duckdb.connect()
    con.execute("SET TimeZone = 'UTC'")
    for t in tables:
        if next((data_dir() / t).glob("*/*/*.parquet"), None) is None:
            con.register(f"{t}_empty", pa.Table.from_pylist([], SCHEMA[t]))
            src = f"{t}_empty"
        else:   # DuckDB expands the glob, so the SQL holds no file list
            g = f"{(data_dir() / t).as_posix()}/*/*/*.parquet".replace("'", "''")
            src = f"read_parquet('{g}', union_by_name = true)"
        con.execute(f"CREATE VIEW {t}_rows AS SELECT * FROM {src}")
        if t in ORDER:   # ties broken by content; an id belongs to one region
            P = SCHEMA[t].names
            k = "region, id" if "region" in P else "id"   # lets region filters push down
            src = (f"(SELECT * FROM {t}_rows QUALIFY row_number() OVER (PARTITION BY {k} "
                   f"ORDER BY {ORDER[t]}, {', '.join(P)}) = 1)")
        con.execute(f"CREATE VIEW {t} AS SELECT * FROM {src}")
    return con


def query(sql, params=()):
    """Rows of `sql` as dicts; only the tables it names are bound."""
    with LOCK, connect([t for t in SCHEMA if re.search(rf"\b{t}(_rows)?\b", sql)]) as con:
        cur = con.execute(sql, params)
        cols = [c[0] for c in cur.description]
        return [dict(zip(cols, r)) for r in cur.fetchall()]
