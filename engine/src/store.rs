//! Append-only Parquet store (spec § Store).
//!
//! Files live at <data>/<table>/<region>/<UTC day written>/<uuid>.parquet. Tables with
//! an id are deduplicated on read (`view`: first by the order column, ties broken by
//! the row's content, so every read picks the same copy), so re-collecting the same
//! item is idempotent (spec § Errors); `rows` is every stored copy. The data root is
//! $OSW_DATA or ./data of the repository. Reads are cached per file.

use std::any::Any;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use chrono::Utc;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::schema::{Entity, Event, Obs, Product, Raw, Run, Status, Table, Track, Unique};

/// Compaction and table swaps delete files; reads must not see that happen.
static LOCK: RwLock<()> = RwLock::new(());
static ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);

type Stamp = (u64, Option<SystemTime>);
type Rows = Arc<dyn Any + Send + Sync>;
/// Per file: (size and mtime, its rows); per table: (listing, all rows, view).
#[derive(Default)]
struct Cache {
    file: HashMap<PathBuf, (Stamp, Rows)>,
    table: HashMap<&'static str, (Vec<(PathBuf, Stamp)>, Rows, Option<Rows>)>,
}
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// The repository: engine/ lies in it.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn data_dir() -> PathBuf {
    if let Some(p) = ROOT.read().unwrap().clone() {
        return p;
    }
    std::env::var_os("OSW_DATA").map_or(repo().join("data"), PathBuf::from)
}

/// Point the store at another data root (tests).
pub fn set_root(p: Option<PathBuf>) {
    *ROOT.write().unwrap() = p;
    *CACHE.lock().unwrap() = None;
}

/// One file per region of rows under root/<region>/<UTC day written>/. The day is
/// the write date, not a row time: observations carry publication times years
/// apart, and one directory per such date left the store as ~1,000 small files.
fn write<T: Table>(root: &Path, rows: &[T]) -> usize {
    let day = Utc::now().format("%Y-%m-%d").to_string();
    let mut parts: Vec<(&str, Vec<T>)> = vec![];
    for r in rows {
        match parts.iter_mut().find(|(k, _)| *k == r.region()) {
            Some((_, v)) => v.push(r.clone()),
            None => parts.push((r.region(), vec![r.clone()])),
        }
    }
    for (region, R) in parts {
        let d = root.join(region).join(&day);
        fs::create_dir_all(&d).unwrap();
        let p = d.join(format!("{}.pending", uuid::Uuid::new_v4().simple()));
        save(&p, &R);
        fs::rename(&p, p.with_extension("parquet")).unwrap();
    }
    rows.len()
}

fn save<T: Table>(p: &Path, R: &[T]) {
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let mut w = ArrowWriter::try_new(File::create(p).unwrap(), T::schema(), Some(props)).unwrap();
    w.write(&T::batch(R)).unwrap();
    w.close().unwrap();
}

fn load<T: Table>(p: &Path) -> Vec<T> {
    let r = ParquetRecordBatchReaderBuilder::try_new(File::open(p).unwrap())
        .unwrap()
        .build()
        .unwrap();
    r.flat_map(|b| T::rows(&b.unwrap())).collect()
}

/// Write rows to their table. Returns their number.
pub fn append<T: Table>(rows: &[T]) -> usize {
    let _g = LOCK.read().unwrap();
    write(&data_dir().join(T::NAME), rows)
}

/// Rename the table's directory aside, then delete it: on Windows a deleted
/// directory keeps its name while any handle (an indexer, antivirus, file
/// watcher) is open on it, and a new directory cannot take that name until then.
fn discard(table: &str) {
    let d = data_dir().join(table);
    if d.exists() {
        fs::rename(
            &d,
            data_dir().join(format!(".{table}.{}.old", uuid::Uuid::new_v4().simple())),
        )
        .unwrap();
    }
    for e in fs::read_dir(data_dir()).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with(&format!(".{table}.")) && n.ends_with(".old") {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

pub fn drop<T: Table>() {
    let _g = LOCK.write().unwrap();
    discard(T::NAME);
}

/// Swap the table for exactly `rows` (a recomputed derived table), atomically with
/// respect to reads.
pub fn replace<T: Table>(rows: &[T]) -> usize {
    let tmp = data_dir().join(format!(
        ".{}.{}.new",
        T::NAME,
        uuid::Uuid::new_v4().simple()
    ));
    let n = write(&tmp, rows);
    let _g = LOCK.write().unwrap();
    discard(T::NAME);
    if tmp.exists() {
        fs::rename(&tmp, data_dir().join(T::NAME)).unwrap();
    }
    n
}

fn dirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn files(p: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    v.sort();
    v
}

fn compact_table<T: Table>(before: &str) -> usize {
    let mut n = 0;
    for r in dirs(&data_dir().join(T::NAME)) {
        let mut G: Vec<(String, Vec<PathBuf>)> = vec![];
        for d in dirs(&r) {
            let name = d.file_name().unwrap().to_string_lossy().into_owned();
            if name.as_str() >= before {
                continue;
            }
            let key = if name.get(..7) < before.get(..7) {
                name[..7].to_string()
            } else {
                name
            };
            match G.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(d),
                None => G.push((key, vec![d])),
            }
        }
        for (key, D) in G {
            let mut F: Vec<PathBuf> = D.iter().flat_map(|d| files(d, "parquet")).collect();
            F.sort();
            if F.len() < 2 {
                continue;
            }
            let rows: Vec<T> = F.iter().flat_map(|f| load::<T>(f)).collect();
            let out = r.join(&key);
            fs::create_dir_all(&out).unwrap();
            save(
                &out.join(format!("{}.parquet", uuid::Uuid::new_v4().simple())),
                &rows,
            );
            for f in &F {
                fs::remove_file(f).unwrap();
            }
            for d in &D {
                if *d.file_name().unwrap() != *key
                    && fs::read_dir(d).is_ok_and(|mut e| e.next().is_none())
                {
                    fs::remove_dir(d).unwrap();
                }
            }
            n += F.len();
        }
    }
    n
}

/// Merge each <table>/<region>/<day> directory dated before `before` (default:
/// today, UTC) into one file, and every directory of a month before `before`'s
/// month into one file under <table>/<region>/<YYYY-MM>. A row is rewritten at most
/// twice, so a store kept under git holds at most three copies of its data, and the
/// file count grows by month, not by write. Returns the files removed.
pub fn compact(before: Option<&str>) -> usize {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let b = before.unwrap_or(&today);
    let _g = LOCK.write().unwrap();
    compact_table::<Run>(b)
        + compact_table::<Raw>(b)
        + compact_table::<Obs>(b)
        + compact_table::<Event>(b)
        + compact_table::<Entity>(b)
        + compact_table::<Status>(b)
        + compact_table::<Product>(b)
        + compact_table::<Track>(b)
}

fn listing(table: &str) -> Vec<(PathBuf, Stamp)> {
    let mut v = vec![];
    for r in dirs(&data_dir().join(table)) {
        for d in dirs(&r) {
            for f in files(&d, "parquet") {
                let m = fs::metadata(&f).ok();
                v.push((
                    f,
                    m.as_ref()
                        .map_or((0, None), |m| (m.len(), m.modified().ok())),
                ));
            }
        }
    }
    v
}

/// Every stored row of the table, duplicates included, in file order.
pub fn rows<T: Table>() -> Arc<Vec<T>> {
    let _g = LOCK.read().unwrap();
    let L = listing(T::NAME);
    let mut c = CACHE.lock().unwrap();
    let c = c.get_or_insert_with(Cache::default);
    if let Some((l, R, _)) = c.table.get(T::NAME)
        && *l == L
    {
        return R.clone().downcast::<Vec<T>>().unwrap();
    }
    let mut all = vec![];
    for (f, s) in &L {
        let hit = c
            .file
            .get(f)
            .filter(|(t, _)| t == s)
            .map(|(_, r)| r.clone());
        let R = hit.unwrap_or_else(|| {
            let r: Rows = Arc::new(load::<T>(f));
            c.file.insert(f.clone(), (*s, r.clone()));
            r
        });
        all.extend(R.downcast_ref::<Vec<T>>().unwrap().iter().cloned());
    }
    let keep: std::collections::HashSet<&PathBuf> = L.iter().map(|(f, _)| f).collect();
    c.file
        .retain(|f, _| keep.contains(f) || !f.starts_with(data_dir().join(T::NAME)));
    let all = Arc::new(all);
    c.table.insert(T::NAME, (L, all.clone(), None));
    all
}

/// One row per key (`Unique::first` of its stored copies), in key order.
pub fn view<T: Unique>() -> Arc<Vec<T>> {
    let R = rows::<T>();
    {
        let c = CACHE.lock().unwrap();
        if let Some((_, r, Some(v))) = c.as_ref().and_then(|c| c.table.get(T::NAME))
            && Arc::ptr_eq(&r.clone().downcast::<Vec<T>>().unwrap(), &R)
        {
            return v.clone().downcast::<Vec<T>>().unwrap();
        }
    }
    let mut o: Vec<&T> = R.iter().collect();
    o.sort_by(|a, b| a.key().cmp(&b.key()).then_with(|| a.first(b)));
    let mut v: Vec<T> = vec![];
    for x in o {
        if v.last().is_none_or(|y| y.key() != x.key()) {
            v.push(x.clone());
        }
    }
    let v = Arc::new(v);
    if let Some(e) = CACHE
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|c| c.table.get_mut(T::NAME))
        && Arc::ptr_eq(&e.1.clone().downcast::<Vec<T>>().unwrap(), &R)
    {
        e.2 = Some(v.clone());
    }
    v
}
