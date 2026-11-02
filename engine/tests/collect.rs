//! Adapters on recorded responses, URL building, and failure as a run status.
#![allow(non_snake_case)]

mod common;

use common::*;
use osw::collect::{self, Fail, Item, Resp, Source, adsb, alerts, firms, markets, neptun};
use osw::collect::{rss, telegram};
use osw::{cycle, regions};
use serde_json::{Value, json};

fn named(name: &str) -> Source {
    Source {
        name: name.into(),
        ..Default::default()
    }
}

fn items(file: &str, name: &str) -> Vec<Item> {
    rss::parse(&fix(file), &named(name)).unwrap()
}

fn kind(k: &str) -> Source {
    Source {
        kind: k.into(),
        ..Default::default()
    }
}

#[test]
fn rss_fixture() {
    let X = items("bbc_rss.xml", "BBC");
    assert_eq!(
        X[2].payload["title"],
        "Houthi missile strike on Yemen camp kills 73 security recruits, local authorities say"
    );
    assert_eq!(X[0].published_at, Some(at(2026, 10, 20, 8, 53, 6)));
}

#[test]
fn google_news_outlet_suffix_is_split() {
    let p = &items("google_news.xml", "Google News")[0].payload;
    assert_eq!(
        p["title"],
        "Benjamin Netanyahu pushes to resume strikes against Iran"
    );
    assert_eq!(p["outlet"], "Financial Times");
}

/// RFC 4287 §1.1: the minimal single-entry Atom document.
#[test]
fn atom_rfc4287_example() {
    let X = items("rfc4287_example.xml", "X");
    assert_eq!(X.len(), 1);
    assert_eq!(X[0].payload["title"], "Atom-Powered Robots Run Amok");
    assert_eq!(
        X[0].url.as_deref(),
        Some("http://example.org/2003/12/13/atom03")
    );
    assert_eq!(X[0].published_at, Some(at(2003, 12, 13, 18, 30, 2)));
}

#[test]
fn excerpt_bound() {
    let mut X = items("meduza.xml", "Meduza");
    X.extend(telegram::parse(&fix("telegram_DeepStateUA.html"), &Source::default()).unwrap());
    assert!(!X.is_empty());
    for x in &X {
        let s = match x.payload["title"].as_str().filter(|s| !s.is_empty()) {
            Some(t) => t,
            None => x.payload["text"].as_str().unwrap(),
        };
        assert!(s.chars().count() <= collect::EXCERPT);
    }
}

#[test]
fn telegram_preview_page() {
    let X = telegram::parse(&fix("telegram_DeepStateUA.html"), &Source::default()).unwrap();
    let ids: Vec<&str> = X.iter().map(|x| x.native_id.as_str()).collect();
    assert_eq!(ids, ["23911", "23912", "23913"]);
    assert_eq!(X[0].url.as_deref(), Some("https://t.me/DeepStateUA/23911"));
    assert_eq!(X[0].published_at, Some(at(2026, 10, 19, 11, 0, 43)));
    let empty = telegram::parse(
        b"<html><div class='tgme_page'></div></html>",
        &Source::default(),
    );
    assert!(empty.unwrap().is_empty());
}

#[test]
fn firms_keeps_detections_inside_some_region() {
    // 4 Middle East + 3 Eastern Europe; 2 southern-hemisphere dropped
    let X = firms::parse(&fix("firms.csv"), &Source::default()).unwrap();
    assert_eq!(X.len(), 7);
    for x in &X {
        let f = |k: &str| x.payload[k].as_str().unwrap().parse::<f64>().unwrap();
        assert!(
            regions::all()
                .iter()
                .any(|r| r.in_bbox(f("latitude"), f("longitude")))
        );
    }
}

#[test]
fn adsb_military_feed_is_clipped_to_regions() {
    let src = Source {
        region: "global".into(),
        ..Default::default()
    };
    let X = adsb::parse(&fix("adsb_mil.json"), &src).unwrap();
    let ids: std::collections::HashSet<&str> = X.iter().map(|x| x.native_id.as_str()).collect();
    assert_eq!((X.len(), ids.len()), (3, 3));
}

#[test]
fn alerts_providers() {
    let X = alerts::parse(&fix("alertsua_states.json"), &kind("alertsua")).unwrap();
    let L: Vec<&Value> = X.iter().map(|x| &x.payload["locations"]).collect();
    assert_eq!(L, [&json!(["Dnipropetrovsk oblast"])]);
    let Y = alerts::parse(&fix("tzevaadom_synthetic.json"), &kind("tzevaadom")).unwrap();
    assert_eq!(Y.len(), 1);
    assert_eq!(Y[0].payload["locations"], json!(["שדרות", "אשקלון"]));
    assert!(alerts::parse(b"[]", &kind("tzevaadom")).unwrap().is_empty());
}

#[test]
fn neptun_tracks() {
    let X = neptun::parse(&fix("neptun.json"), &Source::default()).unwrap();
    assert_eq!(X.len(), 4);
    assert!(X.iter().all(|x| x.payload["lat"].is_f64())); // markers
}

#[test]
fn markets_parse() {
    let Y = markets::parse(&fix("yahoo_LMT.json"), &kind("yahoo")).unwrap();
    assert_eq!(Y.len(), 1);
    assert_eq!(Y[0].payload["symbol"], "LMT");
    assert_eq!(
        markets::parse(&fix("coingecko.json"), &kind("coingecko"))
            .unwrap()
            .len(),
        4
    );
}

/// A market is kept for a region iff upstream's rule, evaluated by JavaScript, keeps it:
/// keywords match and exclude does not (src/app/api/polymarket).
#[test]
fn polymarket_filter_equals_javascript() {
    let js = "const fs=require('fs');const Q=JSON.parse(fs.readFileSync('tests/fixtures/\
              polymarket.json','utf8')).map(m=>m.question);const out={};for(const r of \
              ['iran-israel','russia-ukraine']){const c=JSON.parse(fs.readFileSync(`regions/${r}/\
              conflict.json`,'utf8'));const K=new RegExp(c.polymarketKeywords.regex,\
              c.polymarketKeywords.flags),X=new RegExp(c.polymarketExclude.regex,\
              c.polymarketExclude.flags);out[r]=Q.filter(q=>K.test(q)&&!X.test(q))}\
              console.log(JSON.stringify(out))";
    let Some(o) = node(&["-e", js]) else {
        eprintln!("node not installed: skipped");
        return;
    };
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let want: Value = serde_json::from_slice(&o.stdout).unwrap();
    let P = markets::parse(&fix("polymarket.json"), &kind("polymarket")).unwrap();
    for (r, Q) in want.as_object().unwrap() {
        let got: Vec<&Value> = P
            .iter()
            .filter(|p| p.payload["regions"].as_array().unwrap().contains(&json!(r)))
            .map(|p| &p.payload["question"])
            .collect();
        assert_eq!(got, Q.as_array().unwrap().iter().collect::<Vec<_>>(), "{r}");
    }
}

#[test]
fn encode_uri_component_equals_javascript() {
    let S = [
        "Iran Israel war",
        "Ukraine \"front line\" OR offensive",
        "a/b?c=d&e",
        "שדרות",
        "~*'()!",
    ];
    let code = format!(
        "console.log(JSON.stringify({}.map(encodeURIComponent)))",
        json!(S)
    );
    let Some(o) = node(&["-e", &code]) else {
        eprintln!("node not installed: skipped");
        return;
    };
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let want: Vec<String> = serde_json::from_slice(&o.stdout).unwrap();
    let got: Vec<String> = S.iter().map(|s| collect::encode_uri_component(s)).collect();
    assert_eq!(got, want);
}

#[test]
fn urls_are_built_as_upstream_builds_them() {
    let gnews =
        |q: &str| format!("https://news.google.com/rss/search?q={q}&hl=en-US&gl=US&ceid=US:en");
    let urls = |R: &str| -> std::collections::HashMap<String, String> {
        cycle::sources(&[R.to_string()], &[], false)
            .into_iter()
            .map(|s| (s.id, s.url))
            .collect()
    };
    let S = urls("iran-israel");
    let c = &regions::get("iran-israel").unwrap().conflict;
    // strikes insert the query raw, conflicts apply encodeURIComponent
    assert_eq!(
        S["gnews:strike#1"],
        gnews(c["strikeQueries"][0].as_str().unwrap())
    );
    assert_eq!(
        S["gnews:conflict#1"],
        gnews(&collect::encode_uri_component(
            c["conflictQueries"][0].as_str().unwrap()
        ))
    );
    let f = &c["flightsCenter"];
    let p = |k: &str| collect::pystr(&f[k]);
    assert_eq!(
        S["adsb:adsb.lol#iran-israel"],
        format!(
            "https://api.adsb.lol/v2/lat/{}/lon/{}/dist/{}",
            p("lat"),
            p("lon"),
            p("dist")
        )
    );
    let M = urls("global");
    assert!(M["markets:Yahoo Finance#^GSPC"].ends_with("chart/%5EGSPC?interval=1d&range=5d"));
    assert!(M["markets:Yahoo Finance#CL=F"].ends_with("chart/CL=F?interval=1d&range=5d"));
}

fn bbc() -> Vec<Source> {
    vec![Source {
        id: "rss:BBC".into(),
        region: "iran-israel".into(),
        name: "BBC".into(),
        ..Default::default()
    }]
}

/// One poll of S answered by `r` (a body, or the failure of the fetch):
/// (status, n_rows, raw reports) of the first run.
fn poll(S: &[Source], r: Result<Vec<u8>, Fail>) -> (String, i32, usize) {
    let (_, raw, runs) = collect::run(S, rss::parse, "test", |_| {
        r.map(|body| Resp {
            status: 200,
            ctype: None,
            body,
        })
    });
    (runs[0].status.clone(), runs[0].n_rows, raw.len())
}

#[test]
fn failure_is_a_status_not_an_exception() {
    let cases: [(Result<Vec<u8>, Fail>, &str); 5] = [
        (Ok(fix("bbc_rss.xml")), "ok"),
        (Ok(b"<rss><channel/></rss>".to_vec()), "empty"),
        (Err(Fail::Blocked), "blocked"), // 401, 403, 429
        (Err(Fail::Http), "http_error"), // 5xx, timeout
        (Ok(b"<!DOCTYPE html><html>".to_vec()), "parse_error"),
    ];
    for (r, status) in cases {
        let (s, n, raw) = poll(&bbc(), r);
        assert_eq!((s.as_str(), n as usize), (status, raw));
    }
}

#[test]
fn one_fetch_serves_every_source_on_the_url() {
    let mut S = bbc();
    S.push(Source {
        region: "russia-ukraine".into(),
        ..S[0].clone()
    });
    let (_, raw, runs) = collect::run(&S, rss::parse, "test", |_| {
        Ok(Resp {
            status: 200,
            ctype: None,
            body: fix("bbc_rss.xml"),
        })
    });
    assert_eq!(runs.iter().map(|r| r.n_rows).collect::<Vec<_>>(), [3, 3]);
    let ids: std::collections::HashSet<&String> = raw.iter().map(|x| &x.id).collect();
    assert_eq!(ids.len(), 6); // one report per region per item
}

#[test]
fn raw_ids_are_content_hashes() {
    let go = || {
        collect::run(&bbc(), rss::parse, "test", |_| {
            Ok(Resp {
                status: 200,
                ctype: None,
                body: fix("bbc_rss.xml"),
            })
        })
    };
    let ids = |r: Vec<osw::schema::Raw>| r.into_iter().map(|x| x.id).collect::<Vec<_>>();
    assert_eq!(ids(go().1), ids(go().1));
}
