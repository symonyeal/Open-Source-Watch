//! Append-only Parquet store (spec § Store).
//!
//! Files live at <data>/<table>/<region>/<UTC day written>/<uuid>.parquet. Tables with
//! an id are deduplicated on read (`view`: first by the order column, ties broken by
//! the row's content, so every read picks the same copy), so re-collecting the same
//! item is idempotent (spec § Errors); `rows` is every stored copy. The data root is
//! $OSW_DATA or ./data of the repository. Reads are cached per file.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use chrono::Utc;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::schema::{Entity, Event, Obs, Product, Raw, Run, Status, Table, Track, Unique};

static ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);
static WRITER: Mutex<()> = Mutex::new(());

#[derive(Clone, Default, Serialize, Deserialize)]
struct Generation {
    id: String,
    parent: Option<String>,
    #[serde(with = "slash")]
    tables: BTreeMap<String, Vec<PathBuf>>,
    inputs: BTreeMap<String, String>,
    #[serde(default)]
    materialized: BTreeMap<String, String>,
}

/// Manifest paths are stored `/`-joined and read per component, so a manifest
/// written on one OS (or with `\`) reads on the other.
mod slash {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        t: &BTreeMap<String, Vec<PathBuf>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_map(t.iter().map(|(k, v)| {
            let v: Vec<String> = v
                .iter()
                .map(|p| {
                    let c: Vec<_> = p.iter().map(|c| c.to_string_lossy()).collect();
                    c.join("/")
                })
                .collect();
            (k, v)
        }))
    }

    pub fn deserialize<'a, D: Deserializer<'a>>(
        d: D,
    ) -> Result<BTreeMap<String, Vec<PathBuf>>, D::Error> {
        let t = BTreeMap::<String, Vec<String>>::deserialize(d)?;
        Ok(t.into_iter()
            .map(|(k, v)| {
                let v = v
                    .iter()
                    .map(|p| p.split(['/', '\\']).collect::<PathBuf>())
                    .collect();
                (k, v)
            })
            .collect())
    }
}

thread_local! {
    static READ: RefCell<Option<Generation>> = const { RefCell::new(None) };
    static WRITE: RefCell<Option<Generation>> = const { RefCell::new(None) };
}

struct Scope(bool);
impl Drop for Scope {
    fn drop(&mut self) {
        if self.0 {
            WRITE.with(|s| *s.borrow_mut() = None);
        } else {
            READ.with(|s| *s.borrow_mut() = None);
        }
    }
}

fn current() -> Generation {
    let p = data_dir().join("CURRENT.json");
    match fs::read(&p) {
        Ok(b) => serde_json::from_slice(&b).expect("invalid generation manifest"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Generation {
            id: "legacy".into(),
            tables: [
                Run::NAME,
                Raw::NAME,
                Obs::NAME,
                Event::NAME,
                Entity::NAME,
                Status::NAME,
                Product::NAME,
                Track::NAME,
            ]
            .into_iter()
            .map(|t| {
                (
                    t.into(),
                    legacy_listing(t)
                        .into_iter()
                        .map(|(p, _)| p.strip_prefix(data_dir()).unwrap().to_path_buf())
                        .collect(),
                )
            })
            .collect(),
            ..Generation::default()
        },
        Err(e) => panic!("read {}: {e}", p.display()),
    }
}

/// Pin all table reads in f to one committed generation. Nested scopes reuse it.
pub fn snapshot<T>(f: impl FnOnce() -> T) -> T {
    if WRITE.with(|s| s.borrow().is_some()) || READ.with(|s| s.borrow().is_some()) {
        return f();
    }
    READ.with(|s| *s.borrow_mut() = Some(current()));
    let _scope = Scope(false);
    f()
}

/// The generation visible to this request or transformation.
pub fn generation() -> String {
    WRITE
        .with(|s| s.borrow().as_ref().map(|g| g.id.clone()))
        .or_else(|| READ.with(|s| s.borrow().as_ref().map(|g| g.id.clone())))
        .unwrap_or_else(|| current().id)
}

fn publish(g: &Generation) {
    let root = data_dir();
    let dir = root.join(".generations");
    fs::create_dir_all(&dir).unwrap();
    let bytes = serde_json::to_vec(g).unwrap();
    let mut file = File::create(dir.join(format!("{}.json", g.id))).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    let pending = root.join(format!(".{}.pending", g.id));
    let mut file = File::create(&pending).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    fs::rename(pending, root.join("CURRENT.json")).expect("publish generation pointer");
}

/// Stage related table changes and publish one manifest. A panic leaves the old
/// generation readable. The OS file lock rejects independent writers; nested
/// transactions join the outer one. Retained immutable files permit recovery.
pub fn transaction<T>(f: impl FnOnce() -> T) -> T {
    if WRITE.with(|s| s.borrow().is_some()) {
        return f();
    }
    assert!(
        !READ.with(|s| s.borrow().is_some()),
        "cannot write a pinned read snapshot"
    );
    let _writer = WRITER.lock().unwrap_or_else(|e| e.into_inner());
    fs::create_dir_all(data_dir()).unwrap();
    let lease = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data_dir().join(".writer.lock"))
        .unwrap();
    lease.try_lock().expect("data root already has a writer");
    let mut g = current();
    if g.id == "legacy" {
        publish(&g);
    }
    g.parent = Some(g.id.clone());
    g.id = uuid::Uuid::new_v4().simple().to_string();
    WRITE.with(|s| *s.borrow_mut() = Some(g));
    let _scope = Scope(true);
    let out = f();
    WRITE.with(|s| publish(s.borrow().as_ref().unwrap()));
    out
}

/// Preserve the exact code, configuration, labels, geography and translation
/// artifacts used by a transformation. Blobs are content addressed and immutable.
pub fn inputs() {
    fn visit(base: &Path, p: &Path, out: &mut BTreeMap<String, String>) {
        if p.is_dir() {
            for p in dirs(p).into_iter().chain(files(p, "rs")) {
                visit(base, &p, out);
            }
            // Resource directories contain mixed extensions.
            for e in fs::read_dir(p).unwrap() {
                let p = e.unwrap().path();
                if p.is_file() && p.extension().is_none_or(|x| x != "rs") {
                    visit(base, &p, out);
                }
            }
        } else if p.exists() {
            let b = fs::read(p).unwrap();
            let hash = Sha256::digest(&b)
                .iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>();
            let blob = data_dir().join(".inputs").join(&hash);
            if !blob.exists() {
                fs::create_dir_all(blob.parent().unwrap()).unwrap();
                let pending =
                    blob.with_extension(format!("{}.pending", uuid::Uuid::new_v4().simple()));
                let mut f = File::create(&pending).unwrap();
                f.write_all(&b).unwrap();
                f.sync_all().unwrap();
                std::mem::drop(f);
                fs::rename(pending, &blob).unwrap();
            }
            out.insert(
                p.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                hash,
            );
        }
    }
    let mut out = BTreeMap::new();
    for p in [
        "engine/src",
        "engine/Cargo.toml",
        "engine/Cargo.lock",
        "regions",
        "docs/labels",
    ] {
        visit(&repo(), &repo().join(p), &mut out);
    }
    visit(
        &data_dir(),
        &data_dir().join("cache/translate.jsonl"),
        &mut out,
    );
    WRITE.with(|s| {
        s.borrow_mut()
            .as_mut()
            .expect("inputs require a transaction")
            .inputs = out
    });
}

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
    if let Some(p) = std::env::var_os("OSW_ROOT") {
        return PathBuf::from(p);
    }
    let cwd = std::env::current_dir().expect("working directory");
    cwd.ancestors()
        .find(|p| p.join("regions").is_dir() && p.join("docs/labels").is_dir())
        .expect("set OSW_ROOT to the packaged resource directory")
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
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// One file per region of rows under root/<region>/<UTC day written>/. The day is
/// the write date, not a row time: observations carry publication times years
/// apart, and one directory per such date left the store as ~1,000 small files.
fn write<T: Table>(root: &Path, rows: &[T]) -> Vec<PathBuf> {
    let mut paths = vec![];
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
        let path = p.with_extension("parquet");
        fs::rename(&p, &path).unwrap();
        paths.push(path.strip_prefix(data_dir()).unwrap().to_path_buf());
    }
    paths
}

fn save<T: Table>(p: &Path, R: &[T]) {
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let mut w = ArrowWriter::try_new(File::create(p).unwrap(), T::schema(), Some(props)).unwrap();
    w.write(&T::batch(R)).unwrap();
    w.close().unwrap();
    File::options()
        .write(true)
        .open(p)
        .unwrap()
        .sync_all()
        .unwrap();
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
    transaction(|| {
        let paths = write(&data_dir().join(T::NAME), rows);
        WRITE.with(|s| {
            let mut s = s.borrow_mut();
            let g = s.as_mut().unwrap();
            g.tables.entry(T::NAME.into()).or_default().extend(paths);
            g.materialized.insert(T::NAME.into(), g.id.clone());
        });
        rows.len()
    })
}

/// Remove a table from the next generation, retaining previous files for recovery.
pub fn drop<T: Table>() {
    transaction(|| {
        WRITE.with(|s| {
            let mut s = s.borrow_mut();
            let g = s.as_mut().unwrap();
            g.tables.insert(T::NAME.into(), vec![]);
            g.materialized.insert(T::NAME.into(), g.id.clone());
        })
    });
}

/// Replace a table in the staged generation without disturbing current readers.
pub fn replace<T: Table>(rows: &[T]) -> usize {
    transaction(|| {
        let paths = write(&data_dir().join(T::NAME), rows);
        WRITE.with(|s| {
            let mut s = s.borrow_mut();
            let g = s.as_mut().unwrap();
            g.tables.insert(T::NAME.into(), paths);
            g.materialized.insert(T::NAME.into(), g.id.clone());
        });
        rows.len()
    })
}

fn dirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match fs::read_dir(p) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(e) => panic!("list {}: {e}", p.display()),
    }
    .map(|e| e.expect("directory entry"))
    .map(|e| e.path())
    .filter(|p| p.is_dir())
    .collect();
    v.sort();
    v
}

fn files(p: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match fs::read_dir(p) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(e) => panic!("list {}: {e}", p.display()),
    }
    .map(|e| e.expect("directory entry"))
    .map(|e| e.path())
    .filter(|p| p.extension().is_some_and(|e| e == ext))
    .collect();
    v.sort();
    v
}

fn compact_table<T: Table>(before: &str) -> usize {
    let root = data_dir();
    let mut groups: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for (p, _) in listing(T::NAME) {
        let day = p.parent().unwrap();
        let name = day.file_name().unwrap().to_str().unwrap();
        if name >= before {
            continue;
        }
        let key = if name.get(..7) < before.get(..7) {
            &name[..7]
        } else {
            name
        };
        groups
            .entry(day.parent().unwrap().join(key))
            .or_default()
            .push(p);
    }
    let mut n = 0;
    for (dir, paths) in groups.into_iter().filter(|(_, p)| p.len() > 1) {
        let rows: Vec<T> = paths.iter().flat_map(|p| load::<T>(p)).collect();
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{}.pending", uuid::Uuid::new_v4().simple()));
        save(&p, &rows);
        let file = p.with_extension("parquet");
        fs::rename(p, &file).unwrap();
        WRITE.with(|s| {
            let mut s = s.borrow_mut();
            let files = s.as_mut().unwrap().tables.get_mut(T::NAME).unwrap();
            files.retain(|p| !paths.contains(&root.join(p)));
            files.push(file.strip_prefix(&root).unwrap().to_path_buf());
        });
        n += paths.len();
    }
    n
}

/// Consolidate visible files; old generations remain recoverable. Physical garbage
/// collection is separate from semantic compaction. Only partitions before the
/// supplied UTC date are consolidated; past months roll up by region.
pub fn compact(before: Option<&str>) -> usize {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let b = before.unwrap_or(&today);
    transaction(|| {
        compact_table::<Run>(b)
            + compact_table::<Raw>(b)
            + compact_table::<Obs>(b)
            + compact_table::<Event>(b)
            + compact_table::<Entity>(b)
            + compact_table::<Status>(b)
            + compact_table::<Product>(b)
            + compact_table::<Track>(b)
    })
}

fn listing(table: &str) -> Vec<(PathBuf, Stamp)> {
    let g = WRITE
        .with(|s| s.borrow().clone())
        .or_else(|| READ.with(|s| s.borrow().clone()))
        .unwrap_or_else(current);
    g.tables
        .get(table)
        .into_iter()
        .flatten()
        .map(|p| {
            let p = data_dir().join(p);
            let m = fs::metadata(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            let stamp = (m.len(), m.modified().ok());
            (p, stamp)
        })
        .collect()
}

fn legacy_listing(table: &str) -> Vec<(PathBuf, Stamp)> {
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
    let L = listing(T::NAME);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
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
        let c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
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
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(|c| c.table.get_mut(T::NAME))
        && Arc::ptr_eq(&e.1.clone().downcast::<Vec<T>>().unwrap(), &R)
    {
        e.2 = Some(v.clone());
    }
    v
}
