//! The region registry: grades, geometry, indicators and the generated TypeScript.
#![allow(non_snake_case)]

mod common;

use std::collections::HashSet;

use common::*;
use osw::collect::{mnd, rss};
use osw::regions::{self, Region};
use osw::schema::KINDS;

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
    each(|r| {
        let I: Vec<_> = r.pir.iter().flat_map(|p| &p.indicator).collect();
        let metric = |i: &&regions::Indicator| i.metric.clone();
        let of = |m: &str| -> Vec<_> {
            I.iter()
                .filter(|i| metric(i).as_deref() == Some(m))
                .collect()
        };
        let E: Vec<_> = I
            .iter()
            .filter(|i| matches!(metric(i).as_deref(), None | Some("event_count")))
            .collect();
        let (T, G, O) = (of("transit"), of("gnss"), of("official"));
        assert!(
            !I.is_empty() && E.len() + T.len() + G.len() + O.len() == I.len(),
            "{}",
            r.id
        );
        assert!(
            G.iter()
                .all(|i| matches!(i.band.as_deref(), Some("medium" | "high")))
        );
        assert!(
            O.iter()
                .all(|i| mnd::FIELDS.contains(&i.field.as_deref().unwrap_or("")))
        );
        for i in &E {
            assert!(KINDS.contains(&i.kind.as_deref().unwrap_or("")), "{}", i.id);
            if i.metric.is_none() {
                let (w, t) = (i.watch.unwrap(), i.trigger.unwrap());
                assert!(1 <= w && w <= t, "{}", i.id);
            }
        }
        for i in &T {
            assert!(
                r.gate.contains_key(i.gate.as_deref().unwrap_or("")),
                "{}",
                i.id
            );
            let (t, w) = (i.trigger_z.unwrap(), i.watch_z.unwrap());
            assert!(t <= w && w < 0.0, "{}", i.id);
        }
        let ids: HashSet<&str> = I.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids.len(), I.len(), "{}", r.id);
        for h in r.pir.iter().flat_map(|p| &p.hypothesis) {
            assert!(
                h.expect.keys().all(|k| ids.contains(k.as_str())),
                "{}",
                h.id
            );
            assert!(
                h.expect
                    .values()
                    .all(|v| matches!(v.as_str(), Some("present" | "absent")))
            );
        }
    });
}

/// src/lib/conflicts/generated.ts equals `osw regions` output.
#[test]
fn generated_typescript_is_fresh() {
    let on_disk = std::fs::read_to_string(regions::ts_path())
        .unwrap()
        .replace("\r\n", "\n");
    assert!(on_disk == regions::build_ts());
}
