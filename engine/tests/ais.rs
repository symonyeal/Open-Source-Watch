//! AIS stream adapter (collect::ais) and its storage path (cycle::ingest). The fixture
//! follows aisstream.io's documented message format.
#![allow(non_snake_case)]

mod common;

use common::*;
use osw::collect::{Source, ais};
use osw::schema::{Entity, Raw, Track};
use osw::{cycle, fuse, regions, store};

fn msgs() -> Vec<String> {
    text("aisstream_position_reports.jsonl")
        .lines()
        .map(String::from)
        .collect()
}

fn src() -> Source {
    ais::sources(regions::all()).remove(0)
}

#[test]
fn positions_only_and_nanosecond_times() {
    let X = ais::parse(msgs().join("\n").as_bytes(), &src()).unwrap();
    let mmsi: Vec<i64> = X
        .iter()
        .map(|x| x.payload["mmsi"].as_i64().unwrap())
        .collect();
    assert_eq!(mmsi, [538009876, 636012345]);
    // 08:00:01.123456789 truncated to the microsecond
    assert_eq!(
        X[0].published_at,
        Some(at(2026, 10, 21, 8, 0, 1) + chrono::Duration::microseconds(123456))
    );
    assert_eq!(X[0].payload["name"], "EXAMPLE TANKER");
}

#[test]
fn subscription_covers_every_gated_region() {
    let s = ais::subscription("k", regions::all());
    let gated = regions::all().iter().filter(|r| !r.gate.is_empty()).count();
    assert_eq!(s["BoundingBoxes"].as_array().unwrap().len(), gated);
    assert!(gated > 0);
    assert_eq!(
        s["FilterMessageTypes"],
        serde_json::json!(["PositionReport"])
    );
}

#[test]
fn without_key_the_stream_is_blocked() {
    let _g = fresh("ais_blocked");
    setenv("AISSTREAM_KEY", None);
    let runs = cycle::ingest(&src(), &msgs(), "test");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "blocked");
    assert_eq!(store::rows::<Raw>().len(), 0);
}

#[test]
fn with_key_messages_become_vessel_tracks() {
    let _g = fresh("ais_tracks");
    setenv("AISSTREAM_KEY", Some("test"));
    let runs = cycle::ingest(&src(), &msgs(), "test");
    assert_eq!((runs[0].status.as_str(), runs[0].n_rows), ("ok", 2));
    let T: Vec<(String, String)> = store::rows::<Track>()
        .iter()
        .map(|x| (x.entity_id.clone(), x.region.clone()))
        .collect();
    for want in [("mmsi:538009876", "red-sea"), ("mmsi:636012345", "hormuz")] {
        assert!(
            T.contains(&(want.0.into(), want.1.into())),
            "{want:?} in {T:?}"
        );
    }
    fuse::fuse();
    let vessels = store::view::<Entity>()
        .iter()
        .filter(|e| e.r#type == "vessel")
        .count();
    assert_eq!(vessels, 2);
    assert_eq!(cycle::ingest(&src(), &[], "test")[0].status, "empty");
}
