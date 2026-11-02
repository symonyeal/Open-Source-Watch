//! News relevance, geocoding, credibility of one observation, language and translation
//! cache.
#![allow(non_snake_case)]

mod common;

use std::collections::BTreeSet;

use common::*;
use osw::process::{self, news};
use osw::regions;
use osw::schema::KINDS;

fn r(id: &str) -> &'static regions::Region {
    regions::get(id).unwrap()
}

#[test]
fn relevance_rule_of_upstream() {
    let K = r("iran-israel").regex("newsRelevanceKeywords");
    assert!(!news::relevant(
        "Iran fans celebrate World Cup win",
        None,
        true,
        &K
    )); // noise first
    assert!(news::relevant("Local weather", None, true, &K)); // unfiltered
    assert!(!news::relevant("Local weather", None, false, &K));
    assert!(news::relevant("Hezbollah statement", None, false, &K));
    assert!(news::relevant("Statement", Some("Gaza"), false, &K)); // category
}

#[test]
fn classifier_predicts_a_known_kind() {
    let k = osw::classifier::predict(&["Russian drones attack Kyiv, two killed".to_string()]);
    assert!(KINDS.contains(&k[0].as_str()), "{}", k[0]);
}

/// GeoNames names match capitalised; 'Hit' (Iraq, under 500,000) is not a place.
#[test]
fn geonames_city_and_short_words() {
    for (rid, s, place) in [
        (
            "iran-israel",
            "Fujairah Oil Zone Hit by Fire After Drone Attack",
            "Fujairah",
        ),
        (
            "russia-ukraine",
            "In the Poltava community, UAVs were hit",
            "Poltava",
        ),
    ] {
        let names = process::geocode(r(rid), s).3;
        assert!(names.iter().any(|n| n == place), "{rid}: {names:?}");
        assert!(!names.iter().any(|n| n == "Hit"), "{rid}: {names:?}");
    }
}

#[test]
fn geocode_prefers_the_most_specific_place() {
    let II = r("iran-israel");
    let (lat, lon, prec, names) = process::geocode(II, "Strike near Isfahan, Iran");
    assert_eq!((lat, lon, prec), (Some(32.65), Some(51.67), "city"));
    assert_eq!(
        names.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["isfahan", "iran"])
    );
    assert_eq!(process::geocode(II, "Nothing here").2, "none");
}

#[test]
fn single_observation_credibility() {
    let II = r("iran-israel");
    assert_eq!(process::credibility_single(II, "rss:BBC"), 3); // B
    assert_eq!(process::credibility_single(II, "rss:PressTV#2"), 4); // D, numbered feed
    assert_eq!(process::credibility_single(II, "rss:Unknown"), 6); // ungraded = F
}

#[test]
fn language_by_script() {
    let L: Vec<&str> = ["hello", "שלום", "مرحبا", "привіт"]
        .map(process::lang)
        .to_vec();
    assert_eq!(L, ["en", "he", "ar", "ru"]);
}

/// A cached translation is served without a request; English text is returned as is.
#[test]
fn translation_is_cached() {
    let _g = fresh("process_translation");
    let f = osw::store::data_dir().join("cache");
    std::fs::create_dir_all(&f).unwrap();
    std::fs::write(f.join("translate.jsonl"), "[\"שלום\", \"Hello\"]\n").unwrap();
    assert_eq!(process::translate("שלום"), "Hello");
    assert_eq!(process::translate("hi"), "hi");
}
