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

/// The classifier is scikit-learn's TfidfVectorizer(sublinear_tf, ngram_range=(1, 2)) +
/// LogisticRegression(C=4, class_weight="balanced", tol=1e-8) fitted on kinds-sample.jsonl
/// (scikit-learn 1.9.1): the fixture holds its kind for each text of kinds-week2.jsonl.
#[test]
fn classifier_equals_scikit_learn_on_unseen_texts() {
    let T: Vec<String> = std::fs::read_to_string(repo().join("docs/labels/kinds-week2.jsonl"))
        .unwrap()
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["text"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    let want = text("kinds_week2_sklearn.txt");
    let got = osw::classifier::predict(&T);
    let diff: Vec<usize> = (0..T.len())
        .filter(|&i| got[i] != want.lines().nth(i).unwrap())
        .collect();
    assert_eq!(T.len(), want.lines().count());
    assert!(diff.is_empty(), "differs at {diff:?}");
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
        let names = process::geocode(r(rid), s).names;
        assert!(names.iter().any(|n| n == place), "{rid}: {names:?}");
        assert!(!names.iter().any(|n| n == "Hit"), "{rid}: {names:?}");
    }
}

#[test]
fn geocode_prefers_the_most_specific_place() {
    let II = r("iran-israel");
    let g = process::geocode(II, "Strike near Isfahan, Iran");
    assert_eq!(
        (g.lat, g.lon, g.precision),
        (Some(32.65), Some(51.67), "city")
    );
    assert_eq!(g.names[0], "isfahan"); // the chosen place first
    assert_eq!(
        g.names.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["isfahan", "iran"])
    );
    assert_eq!(process::geocode(II, "Nothing here").precision, "none");
}

/// Russia/Ukraine has every UA and RU place, oblast and raion (`[gazetteer]`); each
/// case is a rule of process::place, from a reviewed miss (osw-intel plan).
#[test]
fn villages_oblasts_and_raions_roll_up() {
    let RU = r("russia-ukraine");
    let g = |s: &str| process::geocode(RU, s);
    let unit = |s: &str| {
        let x = g(s);
        (x.precision, x.admin1, x.admin2, x.names.first().cloned())
    };
    // A name before a unit word is the unit, never the city of that name.
    let x = g("Russian drones attack Kharkiv region overnight");
    assert_eq!(
        (x.precision, x.admin1.as_deref()),
        ("admin", Some("Kharkiv"))
    );
    // "X region" of a federal city is its oblast; an OVA is the oblast's administration.
    assert_eq!(
        g("Drone attack on the Moscow region").admin1.as_deref(),
        Some("Moscow Oblast")
    );
    assert_eq!(g("Two injured, Dnipropetrovsk OVA said").precision, "admin");
    // A village rolls up to raion and oblast; Russian spellings are alternate names.
    let x = g("Three civilians injured in Shebekino, Belgorod region");
    assert_eq!(
        (x.precision, x.admin1.as_deref()),
        ("city", Some("Belgorod Oblast"))
    );
    assert_eq!(
        g("Missile strike on a market in Kharkov").admin1.as_deref(),
        Some("Kharkiv")
    );
    // A front axis is its town's oblast.
    assert_eq!(
        unit("Heaviest fighting on the Pokrovsk axis").1.as_deref(),
        Some("Donetsk")
    );
    // The first place named wins among equals; a capital that speaks is not a place.
    assert_eq!(
        unit("Russia pounds Kyiv with missiles as Moscow says 808 drones downed")
            .3
            .as_deref(),
        Some("kyiv")
    );
    // People, and villages without a locative word or with an ambiguous name, are not places.
    assert_eq!(g("Volodymyr Zelensky meets envoys").precision, "none");
    assert_eq!(g("Gymnast Yana Peshkova was killed").precision, "none");
    assert_eq!(g("Russian drone strike in Pokrovske").precision, "none");
}

/// A town and a site word are a site (`process::place`, OSM sites); a bare town is not.
#[test]
fn a_town_and_a_site_word_are_a_site() {
    let RU = r("russia-ukraine");
    let site = |s: &str| {
        let x = process::geocode(RU, s);
        (x.precision, x.site, x.admin1)
    };
    let (p, s, a1) = site("Drones attack the Novoshakhtinsk refinery overnight");
    assert_eq!(
        (p, s.as_deref(), a1.as_deref()),
        ("site", Some("Novoshakhtinsk refinery"), Some("Rostov"))
    );
    // The word may follow across "in" or "near"; a CHP plant is a power plant.
    assert_eq!(
        site("Fire at the oil refinery in Ryazan").1.as_deref(),
        Some("Ryazan refinery")
    );
    assert_eq!(
        site("Russian drones hit the CHP plant in Kyiv")
            .1
            .as_deref(),
        Some("Kyiv power plant")
    );
    // A Russian adjective before a site word names its town.
    assert_eq!(
        site("Strike on the Novokuibyshevsky oil refinery")
            .1
            .as_deref(),
        Some("Novokuibyshevsk refinery")
    );
    // A site word in another sentence, and a bare town, make no site.
    assert_eq!(
        site("Fire near a military airfield. In Berezniki windows broke").0,
        "city"
    );
    assert_eq!(site("Explosions heard in Engels").1, None);
}

#[test]
fn a_point_takes_the_units_of_its_nearest_place() {
    let (a1, _) = process::units_at(r("russia-ukraine"), 50.45, 30.52);
    assert_eq!(a1.as_deref(), Some("Kyiv City"));
    assert_eq!(
        process::units_at(r("russia-ukraine"), 30.0, 30.0),
        (None, None)
    );
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

/// A stored text is at most EXCERPT characters, its translation included: a
/// 280-character original can translate longer.
#[test]
fn a_translation_is_an_excerpt() {
    let _g = fresh("process_excerpt");
    let s = "Привет ".repeat(40);
    let long = "hello ".repeat(100);
    let d = osw::store::data_dir().join("cache");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("translate.jsonl"),
        serde_json::json!([s, long]).to_string() + "\n",
    )
    .unwrap();
    let t = process::translate(&s);
    assert_eq!(t.chars().count(), osw::collect::EXCERPT);
    assert!(long.starts_with(&t));
}
