//! osw collect [--region R ...] [--adapter A ...] [--runner local|cron] [--due]
//! osw serve [--port 8790] [--no-collect]   # --no-collect: read-only store, proxy not recorded
//! osw rebuild | retranslate | compact | fuse | regions | validate
//! osw analyse [intsum] [sitrep]   # indicator snapshot and products, after fuse
//! osw gazetteer <GeoNames dir>
//! osw backtest <region> <from YYYY-MM-DD> [to] [--step hours] [--healthy]   # boards over history
#![allow(non_snake_case)]

use std::collections::BTreeMap;

use osw::{collect, cycle, fuse, process, regions, signals, store};

/// Values of flag f up to the next flag.
fn values(a: &[String], f: &str) -> Vec<String> {
    a.iter().position(|x| x == f).map_or(vec![], |i| {
        a[i + 1..]
            .iter()
            .take_while(|x| !x.starts_with("--"))
            .cloned()
            .collect()
    })
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let cmd = a.first().map_or("", String::as_str);
    match cmd {
        "collect" => {
            let S = cycle::sources(&values(&a, "--region"), &values(&a, "--adapter"), true);
            let S = if a.iter().any(|x| x == "--due") {
                cycle::due(S, collect::now())
            } else {
                S
            };
            let runner = values(&a, "--runner")
                .first()
                .cloned()
                .unwrap_or("local".into());
            let runs = cycle::collect(&S, &runner, 16);
            let mut n: BTreeMap<&str, usize> = BTreeMap::new();
            for x in &runs {
                *n.entry(x.status.as_str()).or_default() += 1;
            }
            println!(
                "{n:?} {} items",
                runs.iter().map(|x| x.n_rows as i64).sum::<i64>()
            );
        }
        "serve" => {
            let port = values(&a, "--port")
                .first()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8790);
            let collect = !a.iter().any(|x| x == "--no-collect");
            cycle::READ_ONLY.store(!collect, std::sync::atomic::Ordering::Relaxed);
            osw::api::serve(port, collect);
        }
        "rebuild" => println!("{} raw reports reprocessed", cycle::rebuild()),
        "retranslate" => {
            let (n, k) = cycle::retranslate();
            println!("{k} of {n} untranslated text reports translated");
        }
        "compact" => println!("{} files merged", store::compact(None)),
        "fuse" => println!("{} events", fuse::fuse()),
        "analyse" => {
            let K: Vec<&str> = a[1..].iter().map(String::as_str).collect();
            cycle::analyse(&K, collect::now());
            println!(
                "indicator snapshot stored{}",
                if K.is_empty() {
                    String::new()
                } else {
                    format!(", {}", K.join(", "))
                }
            );
        }
        "backtest" => {
            let r = regions::get(a.get(1).expect("region")).expect("known region");
            let day = |s: &String| {
                let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("YYYY-MM-DD");
                d.and_hms_opt(0, 0, 0).unwrap().and_utc()
            };
            let t0 = day(a.get(2).expect("from"));
            let t1 = a
                .get(3)
                .filter(|x| !x.starts_with("--"))
                .map_or(collect::now(), day);
            let h = values(&a, "--step")
                .first()
                .and_then(|x| x.parse().ok())
                .unwrap_or(6);
            let ok = a.iter().any(|x| x == "--healthy");
            let B = cycle::backtest(r, t0, t1, chrono::Duration::hours(h), ok);
            println!("{}", serde_json::to_string_pretty(&B).unwrap());
        }
        "regions" => {
            std::fs::write(regions::ts_path(), regions::build_ts()).unwrap();
            println!(
                "{} TOML regions written",
                regions::all().iter().filter(|r| r.config.is_some()).count()
            );
        }
        "gazetteer" => println!(
            "{} gazetteer names written",
            process::build_geonames(a.get(1).expect("GeoNames directory"))
        ),
        "validate" => {
            for r in regions::all() {
                let v = signals::reference(r);
                if v.is_null() {
                    println!("{} no UCDP events stored", r.id);
                    continue;
                }
                let p = &v["recall"]["engine_period"];
                let d = |x: &serde_json::Value| {
                    x.as_str()
                        .map_or("None".to_string(), |s| s[..10].to_string())
                };
                println!(
                    "{} UCDP {}..{}, {} events; engine collected {}..{}",
                    r.id,
                    v["first"].as_str().unwrap_or(""),
                    v["last"].as_str().unwrap_or(""),
                    v["events"],
                    d(&p[0]),
                    d(&p[1])
                );
                for x in v["recall"]["grid"].as_array().into_iter().flatten() {
                    println!("   {x}");
                }
            }
        }
        _ => {
            eprintln!(
                "usage: osw collect|serve|rebuild|compact|fuse|analyse|backtest|regions|gazetteer|validate"
            );
            std::process::exit(2);
        }
    }
}
