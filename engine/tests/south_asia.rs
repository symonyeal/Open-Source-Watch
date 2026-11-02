//! The South Asia region: scope, queries, news filter, grades and PIRs.
#![allow(non_snake_case)]

mod common;

use std::collections::BTreeSet;
use std::sync::LazyLock;

use common::*;
use osw::process::news;
use osw::regions;
use serde_json::Value;

const COUNTRIES: [(&str, &str); 8] = [
    ("BGD", "Bangladesh"),
    ("IND", "India"),
    ("PAK", "Pakistan"),
    ("LKA", "Sri Lanka"),
    ("MMR", "Myanmar"),
    ("NPL", "Nepal"),
    ("BTN", "Bhutan"),
    ("MDV", "Maldives"),
];

static REGION: LazyLock<Value> = LazyLock::new(|| {
    let s = std::fs::read_to_string(repo().join("regions/south-asia/region.toml")).unwrap();
    toml::from_str(&s).unwrap()
});

fn names() -> BTreeSet<&'static str> {
    COUNTRIES.iter().map(|c| c.1).collect()
}

fn strings<'a>(v: &'a Value, k: &str) -> BTreeSet<&'a str> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x[k].as_str().unwrap())
        .collect()
}

#[test]
fn scope_and_map_cover_all_eight_countries() {
    let codes: BTreeSet<&str> = REGION["context"]["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(codes, COUNTRIES.iter().map(|c| c.0).collect());
    let client = &REGION["conflict"]["client"];
    assert_eq!(strings(&client["cities"], "country"), names());
    let colours: BTreeSet<&str> = client["cityColors"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(names().is_subset(&colours));
    let zones = strings(&client["timeZones"], "zone");
    for z in [
        "Asia/Dhaka",
        "Asia/Kolkata",
        "Asia/Karachi",
        "Asia/Colombo",
        "Asia/Yangon",
        "Asia/Kathmandu",
        "Asia/Thimphu",
        "Indian/Maldives",
        "UTC",
    ] {
        assert!(zones.contains(z), "{z}");
    }
}

#[test]
fn each_country_has_a_news_query_for_relevant_context() {
    let Q = &REGION["conflict"]["server"]["countryQueries"];
    assert_eq!(strings(Q, "country"), names());
    for q in Q.as_array().unwrap() {
        let text = q["query"].as_str().unwrap().to_lowercase();
        let has = |T: &[&str]| T.iter().any(|t| text.contains(t));
        assert!(
            has(&["port", "shipping", "fuel", "power", "hydro"]),
            "{text}"
        );
        assert!(
            has(&[
                "flood",
                "cyclone",
                "earthquake",
                "security",
                "conflict",
                "protest"
            ]),
            "{text}"
        );
        assert!(has(&["cricket", "football"]), "{text}");
    }
}

#[test]
fn google_queries_are_grouped_with_explicit_geography() {
    let server = &REGION["conflict"]["server"];
    assert_eq!(server["filterGoogleNews"], true);
    assert_eq!(strings(&server["countryQueries"], "country"), names());
    let list = |k: &str| -> Vec<&str> {
        server[k]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| q.as_str().unwrap())
            .collect()
    };
    let (strike, conflict) = (list("strikeQueries"), list("conflictQueries"));
    for q in strike.iter().chain(&conflict) {
        assert!(q.starts_with('(') && q.ends_with(')'), "{q}");
        assert!(q.matches('(').count() >= 2, "{q}");
        for c in names() {
            assert!(q.contains(c.split(' ').next().unwrap()), "{c} in {q}");
        }
    }
    assert!(strike.iter().all(|q| q.contains('+') && q.contains("%22")));
    assert!(
        conflict
            .iter()
            .all(|q| !q.contains('+') && !q.contains("%22"))
    );
    assert!(conflict.iter().all(|q| q.contains("\"Sri Lanka\"")));
}

#[test]
fn google_news_filter_drops_live_off_region_and_entertainment_titles() {
    let r = regions::get("south-asia").unwrap();
    let K = r.regex("newsRelevanceKeywords");
    let countries = r.c("countryQueries");
    let rejected = [
        (
            "gnews:strike#2",
            "Thailand imposes martial law in southern province after attack kills 5 paramilitary troops",
        ),
        (
            "gnews:conflict#1",
            "Bangkok flooding disrupts transport and container movements across the capital",
        ),
        (
            "gnews:conflict#2",
            "Paris extends nighttime curfew for third day amid protests and unrest",
        ),
        ("gnews:country#India", "Typhoon Champi batters Taiwan"),
        (
            "gnews:country#India",
            "Kartik Aaryan's 'Captain India' Set Amid Houthi Conflict in Yemen (Exclusive)",
        ),
        (
            "gnews:country#India",
            "Bangladesh Bank website remains offline for third consecutive day",
        ),
    ];
    for (source, title) in rejected {
        assert!(
            !news::regional_google_news(title, source, &K, countries),
            "{title}"
        );
    }
    assert!(news::regional_google_news(
        "India Orders Rooftop Solar Disconnection Following Nationwide Blackout",
        "gnews:country#India",
        &K,
        countries
    ));
    assert!(news::regional_google_news(
        "Burmese forces report clashes near a regional port",
        "gnews:country#Myanmar",
        &K,
        countries
    ));
}

#[test]
fn proven_and_maritime_feeds_are_graded() {
    let feeds = &REGION["conflict"]["server"]["newsFeeds"];
    assert_eq!(
        strings(feeds, "name"),
        BTreeSet::from(["BBC Asia", "Al Jazeera", "gCaptain", "Splash247"])
    );
    let rel = REGION["reliability"].as_object().unwrap();
    for f in feeds.as_array().unwrap() {
        let g = rel[&format!("rss:{}", f["name"].as_str().unwrap())]
            .as_str()
            .unwrap();
        assert!(g.len() == 1 && "ABCDEF".contains(g));
    }
    for k in [
        "gnews:strike",
        "gnews:conflict",
        "gnews:country",
        "firms:VIIRS",
        "adsb:adsb.lol",
        "ais:aisstream",
        "markets:Yahoo Finance",
        "markets:CoinGecko",
        "markets:Polymarket",
    ] {
        assert!(rel.contains_key(k), "{k}");
    }
}

#[test]
fn pirs_use_event_taxonomy_without_economic_thresholds() {
    let pirs = REGION["pir"].as_array().unwrap();
    assert_eq!(
        strings(&REGION["pir"], "id"),
        BTreeSet::from(["SA-1", "SA-2", "SA-3", "SA-4"])
    );
    let I: Vec<&Value> = pirs
        .iter()
        .flat_map(|p| p["indicator"].as_array().unwrap())
        .collect();
    let kinds: BTreeSet<&str> = I.iter().filter_map(|i| i["kind"].as_str()).collect();
    for k in [
        "maritime_incident",
        "battles",
        "violence_against_civilians",
        "protests",
        "internet_outage",
    ] {
        assert!(kinds.contains(k), "{k}");
    }
    // Descriptive throughout: reporting counts and GNSS cell counts, no alert cutoffs.
    for i in I {
        assert!(
            matches!(i["metric"].as_str(), Some("event_count" | "gnss")),
            "{i}"
        );
        assert!(
            i.get("watch").is_none() && i.get("trigger").is_none(),
            "{i}"
        );
    }
}

#[test]
fn region_registry_can_build_the_conflict_config() {
    let r = regions::get("south-asia").unwrap();
    assert_eq!(r.config.as_ref().unwrap()["label"], "SOUTH ASIA");
    let codes: Vec<&str> = COUNTRIES.iter().map(|c| c.0).collect();
    assert_eq!(r.context.countries, codes);
    r.regex("newsRelevanceKeywords");
}
