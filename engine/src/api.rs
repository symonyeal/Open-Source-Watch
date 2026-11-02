//! Engine HTTP API (spec § Seam).
//!
//! /fetch?url=U     the recording proxy: the latest stored response for an upstream URL
//!                  (src/lib/fetcher.ts forwards here when ENGINE_URL is set)
//! /api/telegram    upstream's TelegramData shape (src/lib/types/api.ts), from the store
//! /api/indicators  PIR and indicator status of a region
//! /api/products    latest INTSUM or SITREP of a region
//! /api/context, /api/signals   recorded energy/trade context; GNSS, outages, MND, UCDP
//! /regions, /observations, /events, /tracks, /sources, /api/sources, /stream
//!                  engine-native views

use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::AtomicBool;
use std::time::{Duration as StdDuration, Instant};

use chrono::Duration;
use serde::Serialize;
use serde_json::{Value, json};
use tiny_http::{Header, Request, Response, Server};

use crate::collect::{now, stored};
use crate::context;
use crate::cycle;
use crate::fmt::{Time, iso};
use crate::regions::{self, Region};
use crate::schema::{Event, Obs, Product, Raw, Track};
use crate::signals;
use crate::store;

/// Longer than a poll takes from fetch to write.
const STREAM_LAG: i64 = 15;

type Res = Response<Box<dyn Read + Send>>;

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap()
}

fn reply(status: u16, ctype: &str, body: Vec<u8>) -> Res {
    let n = body.len();
    Response::new(
        status.into(),
        vec![
            header("Content-Type", ctype),
            header("Cache-Control", "no-cache, no-store, must-revalidate"),
        ],
        Box::new(std::io::Cursor::new(body)) as Box<dyn Read + Send>,
        Some(n),
        None,
    )
}

fn js<T: Serialize>(x: &T) -> Res {
    reply(200, "application/json", serde_json::to_vec(x).unwrap())
}

fn bad(msg: &str) -> Res {
    reply(400, "text/plain; charset=utf-8", msg.as_bytes().to_vec())
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
                {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Query parameters of a request URL, decoded; the first of a repeated key.
fn query(url: &str) -> HashMap<String, String> {
    let mut q = HashMap::new();
    for kv in url
        .split_once('?')
        .map_or("", |x| x.1)
        .split('&')
        .filter(|s| !s.is_empty())
    {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        q.entry(decode(k)).or_insert_with(|| decode(v));
    }
    q
}

fn region_of(q: &HashMap<String, String>) -> Result<&'static Region, Res> {
    let id = q
        .get("region")
        .or(q.get("conflict"))
        .filter(|s| !s.is_empty());
    match id {
        Some(id) => regions::get(id).ok_or_else(|| bad("Unknown region")),
        None => Ok(regions::get("iran-israel").unwrap()),
    }
}

fn since(q: &HashMap<String, String>, hours: i64) -> Result<Time, Res> {
    let Some(s) = q.get("since").filter(|s| !s.is_empty()) else {
        return Ok(now() - Duration::hours(hours));
    };
    let t = crate::fmt::fromiso(s).ok_or_else(|| bad("since must be an ISO 8601 timestamp"))?;
    if s.len() <= 10 || !s[10..].contains(['+', '-', 'Z']) {
        return Err(bad("since must include a timezone"));
    }
    Ok(t)
}

fn limit(q: &HashMap<String, String>) -> Result<usize, Res> {
    let msg = "limit must be an integer from 1 to 10000";
    let n: i64 = q
        .get("limit")
        .map_or(Ok(500), |s| s.trim().parse())
        .map_err(|_| bad(msg))?;
    if (1..=10000).contains(&n) {
        Ok(n as usize)
    } else {
        Err(bad(msg))
    }
}

/// Upstream TelegramData: the latest 3 stored posts per channel, newest first
/// (src/app/api/telegram/route.ts); text is the stored excerpt, translated.
pub fn telegram_data(r: &Region, t: Time) -> Value {
    let raw: HashMap<String, Value> = stored(&["telegram"], &[&r.id], Some(t))
        .into_iter()
        .map(|x| (x.id, x.payload))
        .collect();
    let C: Vec<Value> = r
        .c("telegramChannels")
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut by: Vec<(String, Vec<(&Value, String)>)> = vec![];
    let O = store::view::<Obs>();
    for x in O
        .iter()
        .filter(|x| x.region == r.id && x.source_id.starts_with("telegram:") && x.t > t)
    {
        let Some(p) = raw.get(&x.raw_id) else {
            continue;
        };
        let ch = p["channel"].as_str().unwrap_or("").to_string();
        match by.iter_mut().find(|b| b.0 == ch) {
            Some(b) => b.1.push((p, x.text_en.clone())),
            None => by.push((ch, vec![(p, x.text_en.clone())])),
        }
    }
    let mut posts = vec![];
    for (ch, mut P) in by {
        let c = C.iter().find(|c| c["name"] == ch.as_str()).or_else(|| {
            C.iter().find(|c| {
                c["name"]
                    .as_str()
                    .is_some_and(|n| n.to_lowercase() == ch.to_lowercase())
            })
        });
        let Some(c) = c else { continue };
        P.sort_by_key(|y| -y.0["postId"].as_i64().unwrap_or(0));
        for (p, en) in P.into_iter().take(3) {
            posts.push(json!({"channel": c["name"], "channelLabel": c["label"], "color": c["color"],
                              "postId": p["postId"], "text": en, "date": p["date"],
                              "url": format!("https://t.me/{}/{}", c["name"].as_str().unwrap_or(""), p["postId"])}));
        }
    }
    let key = |p: &Value| {
        (
            p["date"].as_str().unwrap_or("").to_string(),
            p["channel"].as_str().unwrap_or("").to_string(),
            p["postId"].as_i64().unwrap_or(0),
        )
    };
    posts.sort_by_key(|p| std::cmp::Reverse(key(p)));
    json!({"posts": posts, "channels": C.iter().map(|c| c["label"].clone()).collect::<Vec<_>>(), "updated": iso(&now())})
}

/// Server-sent events: each new observation of the region, polled every 5 s. A
/// report's fetched_at is its poll's start, earlier than its write, so the query
/// looks back STREAM_LAG minutes and skips observations already sent.
struct Stream {
    r: &'static Region,
    t: Time,
    sent: HashMap<String, Time>,
    buf: Vec<u8>,
    next: Instant,
}

impl Read for Stream {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.buf.is_empty() {
            std::thread::sleep(self.next.saturating_duration_since(Instant::now()));
            self.next = Instant::now() + StdDuration::from_secs(5);
            let lag = now() - Duration::minutes(STREAM_LAG);
            let after = self.t.max(lag);
            let R = store::view::<Raw>();
            let fetched: HashMap<&str, Time> = R
                .iter()
                .filter(|x| x.fetched_at > after)
                .map(|x| (x.id.as_str(), x.fetched_at))
                .collect();
            let O = store::view::<Obs>();
            let mut X: Vec<(&Obs, Time)> = O
                .iter()
                .filter(|x| x.region == self.r.id)
                .filter_map(|x| fetched.get(x.raw_id.as_str()).map(|f| (x, *f)))
                .collect();
            X.sort_by_key(|x| x.0.t);
            for (x, f) in X {
                if !self.sent.contains_key(&x.id) {
                    self.sent.insert(x.id.clone(), f);
                    self.buf.extend(
                        format!(
                            "event: observation\ndata: {}\n\n",
                            serde_json::to_string(x).unwrap()
                        )
                        .bytes(),
                    );
                }
            }
            self.sent.retain(|_, v| *v > lag);
            if self.buf.is_empty() {
                self.buf.extend(b": keep-alive\n\n");
            }
        }
        let n = out.len().min(self.buf.len());
        out[..n].copy_from_slice(&self.buf[..n]);
        self.buf.drain(..n);
        Ok(n)
    }
}

fn route(path: &str, q: &HashMap<String, String>) -> Result<Res, Res> {
    Ok(match path {
        "/fetch" => match cycle::proxy(q.get("url").map_or("", |s| s)) {
            None => reply(502, "text/plain", vec![]),
            Some(r) => reply(r.status, r.ctype.as_deref().unwrap_or("application/octet-stream"), r.body),
        },
        "/regions" => js(&regions::all().iter()
            .map(|r| json!({"id": r.id, "name": r.name, "bbox": r.bbox,
                            "pirs": r.pir.iter().map(|p| json!({"id": p.id, "question": p.question})).collect::<Vec<_>>()}))
            .collect::<Vec<_>>()),
        "/observations" => {
            let (r, t, n) = (region_of(q)?, since(q, 48)?, limit(q)?);
            let (kind, src) = (q.get("kind").filter(|s| !s.is_empty()), q.get("source").filter(|s| !s.is_empty()));
            let O = store::view::<Obs>();
            let mut v: Vec<&Obs> = O.iter().filter(|x| x.region == r.id && x.t > t && kind.is_none_or(|k| &x.kind == k)
                && src.is_none_or(|s| x.source_id.starts_with(s.as_str()))).collect();
            v.sort_by_key(|x| std::cmp::Reverse(x.t));
            v.truncate(n);
            js(&v)
        }
        "/events" => {
            let (r, t, n) = (region_of(q)?, since(q, 48)?, limit(q)?);
            let kind = q.get("kind").filter(|s| !s.is_empty());
            let E = store::view::<Event>();
            let mut v: Vec<&Event> = E.iter().filter(|e| e.region == r.id && e.t_end > t && kind.is_none_or(|k| &e.kind == k)).collect();
            v.sort_by_key(|e| std::cmp::Reverse(e.t_end));
            v.truncate(n);
            js(&v)
        }
        "/tracks" => {
            let (r, t) = (region_of(q)?, since(q, 6)?);
            let T = store::rows::<Track>();
            let mut v: Vec<&Track> = T.iter().filter(|x| x.region == r.id && x.t > t).collect();
            v.sort_by(|a, b| (&a.entity_id, a.t).cmp(&(&b.entity_id, b.t)));
            js(&v)
        }
        "/api/indicators" => js(&cycle::board(region_of(q)?, now())),
        "/api/products" => {
            let (r, kind) = (region_of(q)?, if q.get("kind").map(String::as_str) == Some("sitrep") { "sitrep" } else { "intsum" });
            let P = store::view::<Product>();
            js(&P.iter().filter(|p| p.region == r.id && p.kind == kind).max_by_key(|p| p.t))
        }
        "/api/sources" | "/sources" => js(&cycle::source_table(region_of(q)?, now())),
        "/api/context" => js(&context::view(region_of(q)?, now())),
        "/api/signals" => {
            let r = region_of(q)?;
            js(&signals::view(r, &cycle::source_table(r, now()), now()))
        }
        "/api/telegram" => js(&telegram_data(region_of(q)?, since(q, 24 * 7)?)),
        "/stream" => {
            let s = Stream { r: region_of(q)?, t: now(), sent: HashMap::new(), buf: vec![], next: Instant::now() };
            Response::new(200.into(), vec![header("Content-Type", "text/event-stream"),
                                           header("Cache-Control", "no-cache, no-store, must-revalidate")],
                          Box::new(s) as Box<dyn Read + Send>, None, None)
        }
        _ => reply(404, "text/plain", b"Not Found".to_vec()),
    })
}

fn handle(rq: Request) {
    let url = rq.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    let q = query(&url);
    let res = std::panic::catch_unwind(|| route(&path, &q).unwrap_or_else(|e| e))
        .unwrap_or_else(|_| reply(500, "text/plain", b"Internal Server Error".to_vec()));
    let _ = rq.respond(res);
}

/// Serve the API on 127.0.0.1:port; with collect, the scheduler polls every source by
/// cadence.
pub fn serve(port: u16, collect: bool) {
    static STOP: AtomicBool = AtomicBool::new(false);
    if collect {
        std::thread::spawn(|| cycle::schedule(&STOP, "local"));
    }
    let server = Server::http(("127.0.0.1", port)).unwrap();
    eprintln!("engine on http://127.0.0.1:{port}");
    for rq in server.incoming_requests() {
        std::thread::spawn(move || handle(rq));
    }
}
