//! Tables of the intelligence chain (spec § Data model).
//!
//!     Source ─< RawReport ─< Observation >─ Event >─< Entity
//!
//! `source` is not stored: it is the region registry (crate::regions). Every other
//! table is a Parquet dataset; rows carry their parent ids, so any observation on
//! the map traces back to the raw report that produced it. Columns are nullable,
//! and a column missing from an older file reads as null.

use std::cmp::Ordering;
use std::sync::Arc;

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, Int8Type, Int16Type, Int32Type, TimestampMicrosecondType};
use arrow_array::{
    ArrayRef, Float64Array, Int8Array, Int16Array, Int32Array, RecordBatch, StringArray,
    TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use chrono::{TimeZone, Utc};
use serde::Serialize;

use crate::fmt::Time;
use crate::fmt::ser::{py, py_opt};

/// A column type: its Arrow type, and conversion to and from an Arrow array.
pub trait Col: Sized {
    fn dtype() -> DataType;
    fn build(v: Vec<&Self>) -> ArrayRef;
    /// Values of a column (cast to `dtype`), or nulls where the file lacks it.
    fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self>;
    /// DuckDB's ascending order, nulls last (the tie-break of deduplication).
    fn cmp(a: &Self, b: &Self) -> Ordering;
}

fn cast(a: Option<&ArrayRef>, t: &DataType) -> Option<ArrayRef> {
    a.map(|a| arrow_cast::cast(a, t).expect("column of an unexpected type"))
}

fn utc() -> DataType {
    DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()))
}

macro_rules! opt_col {
    ($T:ty, $dt:expr, $arr:ty, $prim:ty, $ord:expr) => {
        impl Col for Option<$T> {
            fn dtype() -> DataType {
                $dt
            }
            fn build(v: Vec<&Self>) -> ArrayRef {
                Arc::new(v.into_iter().copied().collect::<$arr>())
            }
            fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
                match cast(a, &$dt) {
                    Some(a) => a.as_primitive::<$prim>().iter().collect(),
                    None => vec![None; n],
                }
            }
            fn cmp(a: &Self, b: &Self) -> Ordering {
                nulls_last(a, b, $ord)
            }
        }
        impl Col for $T {
            fn dtype() -> DataType {
                $dt
            }
            fn build(v: Vec<&Self>) -> ArrayRef {
                <Option<$T> as Col>::build(
                    v.into_iter()
                        .map(|x| Some(*x))
                        .collect::<Vec<_>>()
                        .iter()
                        .collect(),
                )
            }
            fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
                <Option<$T> as Col>::column(a, n)
                    .into_iter()
                    .map(|x| x.unwrap_or_default())
                    .collect()
            }
            fn cmp(a: &Self, b: &Self) -> Ordering {
                $ord(a, b)
            }
        }
    };
}

opt_col!(
    f64,
    DataType::Float64,
    Float64Array,
    Float64Type,
    |x: &f64, y: &f64| x.total_cmp(y)
);
opt_col!(i8, DataType::Int8, Int8Array, Int8Type, |x: &i8, y: &i8| x
    .cmp(y));
opt_col!(
    i16,
    DataType::Int16,
    Int16Array,
    Int16Type,
    |x: &i16, y: &i16| x.cmp(y)
);
opt_col!(
    i32,
    DataType::Int32,
    Int32Array,
    Int32Type,
    |x: &i32, y: &i32| x.cmp(y)
);

fn nulls_last<T>(a: &Option<T>, b: &Option<T>, f: impl Fn(&T, &T) -> Ordering) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => f(x, y),
        (None, None) => Ordering::Equal,
        (None, _) => Ordering::Greater,
        (_, None) => Ordering::Less,
    }
}

impl Col for Option<Time> {
    fn dtype() -> DataType {
        utc()
    }
    fn build(v: Vec<&Self>) -> ArrayRef {
        let a: TimestampMicrosecondArray = v
            .into_iter()
            .map(|t| t.map(|t| t.timestamp_micros()))
            .collect();
        Arc::new(a.with_timezone("UTC"))
    }
    fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
        match cast(a, &utc()) {
            Some(a) => a
                .as_primitive::<TimestampMicrosecondType>()
                .iter()
                .map(|x| x.map(|us| Utc.timestamp_micros(us).unwrap()))
                .collect(),
            None => vec![None; n],
        }
    }
    fn cmp(a: &Self, b: &Self) -> Ordering {
        nulls_last(a, b, |x, y| x.cmp(y))
    }
}

impl Col for Time {
    fn dtype() -> DataType {
        utc()
    }
    fn build(v: Vec<&Self>) -> ArrayRef {
        let o: Vec<Option<Time>> = v.into_iter().map(|t| Some(*t)).collect();
        <Option<Time> as Col>::build(o.iter().collect())
    }
    fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
        <Option<Time> as Col>::column(a, n)
            .into_iter()
            .map(|t| t.unwrap_or_default())
            .collect()
    }
    fn cmp(a: &Self, b: &Self) -> Ordering {
        a.cmp(b)
    }
}

impl Col for Option<String> {
    fn dtype() -> DataType {
        DataType::Utf8
    }
    fn build(v: Vec<&Self>) -> ArrayRef {
        Arc::new(v.into_iter().map(|s| s.as_deref()).collect::<StringArray>())
    }
    fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
        match cast(a, &DataType::Utf8) {
            Some(a) => a
                .as_string::<i32>()
                .iter()
                .map(|s| s.map(String::from))
                .collect(),
            None => vec![None; n],
        }
    }
    fn cmp(a: &Self, b: &Self) -> Ordering {
        nulls_last(a, b, |x, y| x.cmp(y))
    }
}

impl Col for String {
    fn dtype() -> DataType {
        DataType::Utf8
    }
    fn build(v: Vec<&Self>) -> ArrayRef {
        Arc::new(
            v.into_iter()
                .map(|s| Some(s.as_str()))
                .collect::<StringArray>(),
        )
    }
    fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
        <Option<String> as Col>::column(a, n)
            .into_iter()
            .map(|s| s.unwrap_or_default())
            .collect()
    }
    fn cmp(a: &Self, b: &Self) -> Ordering {
        a.cmp(b)
    }
}

impl Col for Vec<String> {
    fn dtype() -> DataType {
        DataType::List(Arc::new(Field::new("element", DataType::Utf8, true)))
    }
    fn build(v: Vec<&Self>) -> ArrayRef {
        let mut b = ListBuilder::new(StringBuilder::new()).with_field(Arc::new(Field::new(
            "element",
            DataType::Utf8,
            true,
        )));
        for x in v {
            for s in x {
                b.values().append_value(s);
            }
            b.append(true);
        }
        Arc::new(b.finish())
    }
    fn column(a: Option<&ArrayRef>, n: usize) -> Vec<Self> {
        match cast(a, &Self::dtype()) {
            Some(a) => a
                .as_list::<i32>()
                .iter()
                .map(|x| {
                    x.map_or(vec![], |x| {
                        x.as_string::<i32>()
                            .iter()
                            .map(|s| s.unwrap_or_default().to_string())
                            .collect()
                    })
                })
                .collect(),
            None => vec![vec![]; n],
        }
    }
    fn cmp(a: &Self, b: &Self) -> Ordering {
        a.cmp(b)
    }
}

/// A stored table: its directory name, schema and row conversion.
pub trait Table: Sized + Clone + Send + Sync + 'static {
    const NAME: &'static str;
    fn schema() -> SchemaRef;
    fn batch(R: &[Self]) -> RecordBatch;
    fn rows(b: &RecordBatch) -> Vec<Self>;
    /// Order of whole rows, column by column.
    fn content(&self, o: &Self) -> Ordering;
    /// Region a row is filed under; "global" for none.
    fn region(&self) -> &str;
}

/// A table deduplicated on read: one row per key, the first by `first`.
pub trait Unique: Table {
    fn key(&self) -> (&str, &str);
    /// The order column, then the whole row (so every read picks the same copy).
    fn first(&self, o: &Self) -> Ordering;
}

macro_rules! table {
    ($S:ident $name:literal $region:tt { $($(#[$m:meta])* $f:ident : $T:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Serialize)]
        pub struct $S { $($(#[$m])* pub $f: $T),* }

        impl Table for $S {
            const NAME: &'static str = $name;
            fn schema() -> SchemaRef {
                Arc::new(Schema::new(vec![$(Field::new(
                    stringify!($f).trim_start_matches("r#"), <$T as Col>::dtype(), true)),*]))
            }
            fn batch(R: &[Self]) -> RecordBatch {
                RecordBatch::try_new(Self::schema(), vec![$(<$T as Col>::build(
                    R.iter().map(|r| &r.$f).collect())),*]).unwrap()
            }
            fn rows(b: &RecordBatch) -> Vec<Self> {
                let n = b.num_rows();
                $(let mut $f = <$T as Col>::column(
                    b.column_by_name(stringify!($f).trim_start_matches("r#")), n).into_iter();)*
                (0..n).map(|_| $S { $($f: $f.next().unwrap()),* }).collect()
            }
            fn content(&self, o: &Self) -> Ordering {
                Ordering::Equal $(.then_with(|| <$T as Col>::cmp(&self.$f, &o.$f)))*
            }
            fn region(&self) -> &str {
                table!(@region self $region)
            }
        }
    };
    (@region $s:ident region) => { if $s.region.is_empty() { "global" } else { &$s.region } };
    (@region $s:ident global) => { "global" };
}

macro_rules! unique {
    ($S:ident $order:ident $($region:ident)?) => {
        impl Unique for $S {
            fn key(&self) -> (&str, &str) {
                (unique!(@r self $($region)?), &self.id)
            }
            fn first(&self, o: &Self) -> Ordering {
                Col::cmp(&self.$order, &o.$order).then_with(|| self.content(o))
            }
        }
    };
    (@r $s:ident region) => { &$s.region };
    (@r $s:ident) => { "" };
}

table!(Run "collection_run" region {
    source_id: String, region: String,
    #[serde(serialize_with = "py")] started_at: Time,
    status: String, n_rows: i32, latency_ms: i32, runner: String,
});

table!(Raw "raw_report" region {
    id: String, source_id: String, region: String,
    #[serde(serialize_with = "py")] fetched_at: Time,
    #[serde(serialize_with = "py_opt")] published_at: Option<Time>,
    url: Option<String>, payload: String,
});

table!(Obs "observation" region {
    id: String, raw_id: String, source_id: String, region: String,
    #[serde(serialize_with = "py")] t: Time,
    lat: Option<f64>, lon: Option<f64>, geo_precision: String, kind: String,
    text_en: String, lang: String, credibility: i8, names: Vec<String>,
    /// Stated event time (process::when), if any.
    #[serde(serialize_with = "py_opt")] t_event: Option<Time>,
});

table!(Event "event" region {
    id: String, region: String,
    #[serde(serialize_with = "py")] t_start: Time,
    #[serde(serialize_with = "py")] t_end: Time,
    lat: f64, lon: f64, kind: String, observation_ids: Vec<String>, n_independent: i16,
    credibility: i8,
});

table!(Entity "entity" global {
    id: String, r#type: String, names: Vec<String>, identifiers: String, attributes: String,
});

table!(Status "indicator_status" region {
    region: String,
    #[serde(serialize_with = "py")] t: Time,
    pir: String, indicator: String, status: String, count: Option<i32>,
});

table!(Product "product" region {
    id: String, region: String, kind: String,
    #[serde(serialize_with = "py")] t: Time,
    period_h: i16, text: String,
});

table!(Track "track_point" region {
    entity_id: String, region: String,
    #[serde(serialize_with = "py")] t: Time,
    lat: f64, lon: f64, heading: Option<f64>, speed: Option<f64>, source_id: String,
});

unique!(Raw fetched_at region);
unique!(Obs t region);
unique!(Event t_start region);
unique!(Entity id);
unique!(Product t region);

pub const GEO_PRECISION: [&str; 5] = ["point", "city", "admin", "country", "none"];

/// ACLED event types (ACLED Codebook 2023 §2), extended (spec § Event taxonomy).
/// `report` is a processed item that no rule classified; fusion ignores it.
pub const KINDS: [&str; 12] = [
    "battles",
    "explosions_remote_violence",
    "violence_against_civilians",
    "protests",
    "riots",
    "strategic_developments",
    "air_raid_alert",
    "maritime_incident",
    "airspace_activity",
    "thermal_anomaly",
    "internet_outage",
    "report",
];
