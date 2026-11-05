//! The region registry: grades, geometry, indicators and the generated TypeScript.
#![allow(non_snake_case)]

mod common;

use common::*;
use osw::collect::rss;
use osw::regions::{self, Region};

const GRADES: &str = "ABCDEF";

fn each(f: impl Fn(&Region)) {
    assert!(regions::all().len() >= 7);
    for r in regions::all() {
        f(r);
    }
}

/// regions/*/conflict.json equals the export of the TypeScript configs.
#[test]
fn conflict_json_is_a_fresh_export() {
    let Some(o) = node(&["--no-warnings", "scripts/export-regions.mjs", "--check"]) else {
        eprintln!("node not installed: skipped");
        return;
    };
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn every_source_is_graded() {
    each(|r| {
        let names = |k: &str, p: &str| -> Vec<String> {
            r.c(k)
                .as_array()
                .unwrap()
                .iter()
                .map(|f| format!("{p}:{}", f["name"].as_str().unwrap()))
                .collect()
        };
        let S = [
            names("newsFeeds", "rss"),
            names("telegramChannels", "telegram"),
        ]
        .concat();
        let ungraded: Vec<&String> = S
            .iter()
            .filter(|s| !r.reliability.contains_key(*s))
            .collect();
        assert!(ungraded.is_empty(), "{}: {ungraded:?}", r.id);
        assert!(
            r.reliability
                .values()
                .all(|g| g.len() == 1 && GRADES.contains(g.as_str()))
        );
        assert!(
            r.owner.keys().all(|k| r.reliability.contains_key(k)),
            "{}",
            r.id
        );
    });
}

#[test]
fn region_geometry_and_rules() {
    each(|r| {
        let [a, b, c, d] = r.bbox;
        assert!(-90.0 <= a && a < b && b <= 90.0 && -180.0 <= c && c < d && d <= 180.0);
        let bb = r.c("firesBBox");
        let box_: Vec<f64> = ["latMin", "latMax", "lonMin", "lonMax"]
            .iter()
            .map(|k| bb[*k].as_f64().unwrap())
            .collect();
        assert_eq!(box_, r.bbox, "{}", r.id);
        r.regex("newsRelevanceKeywords");
        assert_eq!(
            rss::feeds(r).len(),
            r.c("newsFeeds").as_array().unwrap().len()
        );
    });
}

#[test]
fn pir_indicators() {
    each(|r| assert!(r.validate().is_ok(), "{}", r.id));
}

/// src/lib/conflicts/generated.ts equals `osw regions` output.
#[test]
fn generated_typescript_is_fresh() {
    let on_disk = std::fs::read_to_string(regions::ts_path())
        .unwrap()
        .replace("\r\n", "\n");
    assert!(on_disk == regions::build_ts());
}

#[test]
fn production_validation_rejects_malformed_regions() {
    each(|r| assert!(r.validate().is_ok()));
    let base = regions::get("iran-israel").unwrap();
    let mut r = base.clone();
    r.pir[0].indicator[0].metric = Some("event_cout".into());
    assert!(r.validate().unwrap_err().contains("unknown metric"));
    let mut r = base.clone();
    r.pir[0].indicator[0].window_h = Some(0);
    assert!(r.validate().unwrap_err().contains("window_h"));
    let mut r = base.clone();
    r.pir[0].indicator[0].watch = Some(1000);
    assert!(r.validate().unwrap_err().contains("watch <= trigger"));
    let mut r = base.clone();
    r.reliability.insert("rss:bad".into(), "reliable".into());
    assert!(r.validate().unwrap_err().contains("grade A-F"));
    let mut r = base.clone();
    r.bbox[0] = f64::NAN;
    assert!(r.validate().unwrap_err().contains("bbox"));
    let mut r = base.clone();
    r.pir[0].indicator[0].band = Some("high".into());
    assert!(
        r.validate()
            .unwrap_err()
            .contains("incompatible measurement fields")
    );
    let mut r = regions::get("hormuz").unwrap().clone();
    r.gate.clear();
    assert!(r.validate().unwrap_err().contains("unknown gate"));
}
