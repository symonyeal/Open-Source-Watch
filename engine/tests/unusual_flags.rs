//! The unusual-activity backtest of README.md and VALIDATION.md against the judged
//! flags docs/labels/unusual-flags.jsonl. Ignored: it needs a store rebuilt from the
//! `data` branch at be528bf7 (`osw rebuild` on a copy), named by OSW_DATA. Run with
//! `OSW_DATA=<copy> cargo test --release --test unusual_flags -- --ignored`.
#![allow(non_snake_case)]

mod common;

use std::collections::BTreeSet;

use chrono::{Duration, TimeZone, Utc};
use osw::{cycle, regions, store};
use serde_json::Value;

type Flag = (String, String, String);

#[test]
#[ignore]
fn flags_and_supported_flags_kept() {
    common::offline();
    store::set_root(Some(std::env::var_os("OSW_DATA").expect("OSW_DATA").into()));
    let t = |m, d| Utc.with_ymd_and_hms(2026, m, d, 0, 0, 0).unwrap();
    let B = cycle::backtest(
        regions::get("russia-ukraine").unwrap(),
        t(10, 14),
        t(11, 3),
        Duration::hours(6),
        true,
    );
    let mut got: BTreeSet<Flag> = BTreeSet::new();
    for i in B
        .iter()
        .filter(|i| ["RU-2c", "RU-5a", "RU-5b"].contains(&i["id"].as_str().unwrap()))
    {
        for s in i["triggered"].as_array().unwrap() {
            for a in s["areas"].as_array().unwrap() {
                got.insert((
                    i["id"].as_str().unwrap().into(),
                    a["area"].as_str().unwrap().into(),
                    s["t"].as_str().unwrap()[..10].into(),
                ));
            }
        }
    }
    let text =
        std::fs::read_to_string(common::repo().join("docs/labels/unusual-flags.jsonl")).unwrap();
    let mut judged: BTreeSet<Flag> = BTreeSet::new();
    let mut supported: BTreeSet<Flag> = BTreeSet::new();
    for l in text.lines() {
        let f: Value = serde_json::from_str(&l.replace(": NaN", ": null")).unwrap();
        let k = (
            f["indicator"].as_str().unwrap().to_string(),
            f["oblast"].as_str().unwrap().to_string(),
            f["step"].as_str().unwrap()[..10].to_string(),
        );
        let real = f["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["label"] == "real")
            .count();
        if real >= 3 {
            supported.insert(k.clone());
        }
        judged.insert(k);
    }
    let kept = |S: &BTreeSet<Flag>| S.iter().filter(|k| got.contains(*k)).count();
    assert_eq!(
        (
            got.len(),
            kept(&supported),
            supported.len(),
            kept(&judged),
            got.difference(&judged).count()
        ),
        (38, 10, 13, 37, 1)
    );
}
