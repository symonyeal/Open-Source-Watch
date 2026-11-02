//! Public Telegram channels via the web preview t.me/s/<channel>, which lists the
//! latest posts in one request. Upstream (src/app/api/telegram) probes post ids one
//! embed page at a time; the preview needs one request per channel per poll.
//! Channels with the preview disabled return no posts: status `empty`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use super::{EXCERPT, Item, Source, date, excerpt};
use crate::regions::Region;

pub const CADENCE: f64 = 120.0;
static POST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"data-post="([^"/]+)/(\d+)""#).unwrap());
static TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<div class="tgme_widget_message_text js-message_text"[^>]*>(.*?)</div>"#)
        .unwrap()
});
static TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<time[^>]*datetime="([^"]+)""#).unwrap());
static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

pub fn sources(R: &[Region]) -> Vec<Source> {
    R.iter()
        .flat_map(|r| {
            r.c("telegramChannels")
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|c| {
                    let n = c["name"].as_str().unwrap_or("").to_string();
                    Source {
                        id: format!("telegram:{n}"),
                        region: r.id.clone(),
                        url: format!("https://t.me/s/{n}"),
                        cadence: CADENCE,
                        ..Default::default()
                    }
                })
        })
        .collect()
}

/// Plain text of an HTML fragment, as upstream src/lib/telegramText.ts.
pub fn plain(fragment: &str) -> String {
    let s = BR.replace_all(fragment, " ");
    let s = htmlize::unescape(TAG.replace_all(&s, "").as_ref()).to_string();
    SPACE.replace_all(&s, " ").trim().to_string()
}

pub fn parse(body: &[u8], _src: &Source) -> Result<Vec<Item>, String> {
    let page = String::from_utf8_lossy(body);
    if !page.contains("tgme_") {
        return Err("not a Telegram page".into());
    }
    let M: Vec<_> = POST.captures_iter(&page).collect();
    let mut items = vec![];
    for (k, m) in M.iter().enumerate() {
        let end = M
            .get(k + 1)
            .map_or(page.len(), |n| n.get(0).unwrap().start());
        let block = &page[m.get(0).unwrap().end()..end];
        let (ch, pid) = (&m[1], m[2].parse::<i64>().map_err(|e| e.to_string())?);
        let Some(t) = TEXT.captures(block) else {
            continue;
        };
        let s = plain(&t[1]);
        if s.is_empty() {
            continue;
        }
        let when = TIME.captures(block).map(|c| c[1].to_string());
        let url = format!("https://t.me/{ch}/{pid}");
        items.push(Item {
            native_id: pid.to_string(), published_at: date(when.as_deref()), url: Some(url),
            payload: json!({"channel": ch, "postId": pid, "text": excerpt(&s, EXCERPT), "date": when}),
        });
    }
    Ok(items)
}
