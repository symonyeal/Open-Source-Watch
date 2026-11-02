//! RSS 2.0 and Atom feeds: the conflict's newsFeeds (upstream src/app/api/news) and
//! its Google News queries (src/app/api/{strikes,conflicts,regional-alerts}).
//!
//! URLs are built exactly as upstream builds them, so the engine's stored response
//! answers the upstream route's request (spec § Seam): strikes and regional-alerts
//! insert the query raw, conflicts applies encodeURIComponent.

use std::collections::HashMap;

use roxmltree::{Document, Node};
use serde_json::{Value, json};

use super::{EXCERPT, Item, Source, date, encode_uri_component, excerpt};
use crate::regions::Region;
use crate::schema::Raw;
use crate::store;

pub const CADENCE: f64 = 300.0;
/// Upstream reads the first 15 (news, strikes) or 25 (conflicts) items.
const PER_FEED: usize = 25;

fn gnews(q: &str) -> String {
    format!("https://news.google.com/rss/search?q={q}&hl=en-US&gl=US&ceid=US:en")
}

/// (source id, feed) of the region's newsFeeds; feeds sharing a name are numbered
/// name#1, name#2, ...
pub fn feeds(r: &Region) -> Vec<(String, &Value)> {
    let F = r
        .c("newsFeeds")
        .as_array()
        .map_or(&[][..], |v| v.as_slice());
    let name = |f: &Value| f["name"].as_str().unwrap_or("").to_string();
    let mut k: HashMap<String, usize> = HashMap::new();
    F.iter()
        .map(|f| {
            let n = name(f);
            let total = F.iter().filter(|g| name(g) == n).count();
            *k.entry(n.clone()).or_default() += 1;
            let sid = if total > 1 {
                format!("rss:{n}#{}", k[&n])
            } else {
                format!("rss:{n}")
            };
            (sid, f)
        })
        .collect()
}

pub fn sources(R: &[Region]) -> Vec<Source> {
    let accept = vec![(
        "Accept",
        "application/rss+xml, application/xml, text/xml, */*".to_string(),
    )];
    let mut S = vec![];
    for r in R {
        let src =
            |id: String, url: String, name: &str, headers: Vec<(&'static str, String)>| Source {
                id,
                region: r.id.clone(),
                url,
                cadence: CADENCE,
                name: name.into(),
                headers,
                ..Default::default()
            };
        for (sid, f) in feeds(r) {
            S.push(src(
                sid,
                f["url"].as_str().unwrap_or("").into(),
                f["name"].as_str().unwrap_or(""),
                accept.clone(),
            ));
        }
        let list = |k: &str| r.c(k).as_array().cloned().unwrap_or_default();
        for (k, q) in list("strikeQueries").iter().enumerate() {
            S.push(src(
                format!("gnews:strike#{}", k + 1),
                gnews(q.as_str().unwrap_or("")),
                "Google News",
                vec![],
            ));
        }
        for (k, q) in list("conflictQueries").iter().enumerate() {
            S.push(src(
                format!("gnews:conflict#{}", k + 1),
                gnews(&encode_uri_component(q.as_str().unwrap_or(""))),
                "Google News",
                vec![],
            ));
        }
        for q in list("countryQueries") {
            S.push(src(
                format!("gnews:country#{}", q["country"].as_str().unwrap_or("")),
                gnews(q["query"].as_str().unwrap_or("")),
                "Google News",
                vec![],
            ));
        }
    }
    S
}

fn child<'a>(e: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    e.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// The element's own text up to its first child element, stripped (ElementTree's `text`).
fn text_of(e: Node) -> String {
    let mut s = String::new();
    for c in e.children() {
        if c.is_element() {
            break;
        }
        if let Some(t) = c.text() {
            s.push_str(t);
        }
    }
    s.trim().to_string()
}

fn text(e: Node, name: &str) -> String {
    child(e, name).map_or(String::new(), text_of)
}

/// Items of an RSS or Atom document, first PER_FEED, as collection items.
pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let xml = String::from_utf8_lossy(body);
    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = Document::parse_with_options(&xml, opt).map_err(|e| e.to_string())?;
    let all = |n: &str| {
        doc.descendants()
            .filter(|e| e.is_element() && e.tag_name().name() == n)
            .collect::<Vec<_>>()
    };
    let mut E = all("item");
    if E.is_empty() {
        E = all("entry");
    }
    let mut items = vec![];
    for e in E.into_iter().take(PER_FEED) {
        let mut title = text(e, "title");
        if title.is_empty() {
            continue;
        }
        let mut outlet = String::new();
        if let Some(k) = title
            .rfind(" - ")
            .filter(|&k| src.name == "Google News" && k > 0)
        {
            outlet = title[k + 3..].to_string();
            title.truncate(k);
        }
        let mut link = text(e, "link");
        if link.is_empty() {
            link = child(e, "link")
                .and_then(|l| l.attribute("href"))
                .unwrap_or("")
                .to_string();
        }
        let pub_ = [text(e, "pubDate"), text(e, "published"), text(e, "updated")]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default();
        let category = text(e, "category");
        items.push(Item {
            native_id: if link.is_empty() {
                title.clone()
            } else {
                link.clone()
            },
            published_at: date(Some(&pub_)),
            url: Some(link.clone()),
            payload: json!({"title": excerpt(&title, EXCERPT), "link": link, "source": src.name,
                            "pubDate": pub_, "category": (!category.is_empty()).then_some(category),
                            "outlet": (!outlet.is_empty()).then_some(outlet)}),
        });
    }
    Ok(items)
}

/// {raw report id: outlet} of the stored news items that name one (Google News
/// titles "Title - Outlet"), as fusion needs it for every item.
pub fn outlets() -> HashMap<String, String> {
    let mut O = HashMap::new();
    for x in store::view::<Raw>().iter() {
        if !(x.source_id.starts_with("rss:") || x.source_id.starts_with("gnews:"))
            || !x.payload.contains("\"outlet\"")
        {
            continue;
        }
        if let Ok(p) = serde_json::from_str::<Value>(&x.payload)
            && let Some(o) = p["outlet"].as_str()
        {
            O.insert(x.id.clone(), o.to_string());
        }
    }
    O
}
