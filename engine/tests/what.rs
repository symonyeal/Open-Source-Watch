//! Incident detail (process::what): the statement filter and the fields, on cases and
//! on the held-out labelled sample docs/labels/detail-fresh.jsonl (VALIDATION.md).
#![allow(non_snake_case)]

use osw::process::{geocode, what};
use osw::regions;
use serde_json::Value;

fn d(s: &str) -> what::Detail {
    let g = geocode(regions::get("russia-ukraine").unwrap(), s);
    what::detail(s, g.kind, g.country.as_deref())
}

#[test]
fn statements_are_not_incidents() {
    assert!(!d("Zelensky: 2 Ways to Halt Strikes").event);
    assert!(!d("Putin Focused on Medieval History in Talks With Nordic Leaders").event);
    // A weapon named keeps a report the words alone would drop.
    assert!(d("Russia attacks Ukraine with 433 drones: hits recorded at 16 locations").event);
}

#[test]
fn fields_of_a_strike() {
    let x = d("Russian ballistic missile attack on Kyiv kills one, damages residential buildings");
    assert_eq!(x.sub_event, Some("Shelling/artillery/missile attack"));
    assert_eq!(x.weapon, vec!["missile"]);
    assert_eq!(x.missile, Some("ballistic"));
    assert_eq!(x.target, Some("civilian"));
    assert_eq!((x.killed, x.injured), (Some(1), None));
    assert_eq!((x.actor1, x.actor2), (Some("Russia"), Some("Ukraine")));
    // A site's kind decides the target.
    let x = d("Drones attack the Novoshakhtinsk refinery overnight");
    assert_eq!(
        (x.target, x.sub_event),
        (Some("energy"), Some("Air/drone strike"))
    );
}

/// The filter applies to the violent kinds: of the sample's labelled incidents filed as
/// one, it drops 4 of 163; of its statements, 28 of 50 (VALIDATION.md, 2026-11-02).
#[test]
fn statement_filter_on_the_held_out_sample() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/labels/detail-fresh.jsonl"
    );
    let X: Vec<Value> = std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let violent = [
        "battles",
        "explosions_remote_violence",
        "violence_against_civilians",
    ];
    let (mut e, mut fd, mut st, mut sd) = (0, 0, 0, 0);
    for x in X
        .iter()
        .filter(|x| violent.contains(&x["kind"].as_str().unwrap()))
    {
        let drop = !d(x["text"].as_str().unwrap()).event;
        if x["event"].as_bool().unwrap() {
            (e, fd) = (e + 1, fd + drop as i32);
        } else {
            (st, sd) = (st + 1, sd + drop as i32);
        }
    }
    assert_eq!((fd, e, sd, st), (4, 163, 28, 50));
}
