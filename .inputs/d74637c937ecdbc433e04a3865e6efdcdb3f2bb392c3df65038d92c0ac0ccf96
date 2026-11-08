//! News raw reports (rss:*, gnews:*) → observations, with upstream's relevance rule
//! (src/app/api/news/route.ts): sports/entertainment noise is dropped; items from an
//! unfiltered feed or a Google News query pass; otherwise the title or category
//! must match the theater's relevance keywords. Irrelevant reports stay in
//! raw_report with no observation.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::observations;
use crate::collect::{Report, rss};
use crate::regions::{self, Region};
use crate::schema::Obs;

static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)world.?cup|\bfifa\b|\bioc\b|olympic|premier.?league|champions.?league|super.?bowl|\bnba\b|\bnfl\b|\bnhl\b|\bmlb\b|grammy|oscar|\bemmy|box.?office|celebrity|eurovision").unwrap()
});
static REGIONAL_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\bcricket|\bcricketer|\bipl\b|\bfootball\b|\bsoccer\b|\bsport(s)?\b|",
        r"\bfilm\b|\bmovie\b|\bcinema\b|\bactor\b|\bactress\b|\bbollywood\b|",
        r"\bhollywood\b|\btollywood\b|\bbox.?office\b|\btrailer\b|\bcelebrity\b|",
        r"\bentertainment\b|\bstreaming\b|\bnetflix\b|\bkartik aaryan\b|",
        r"\bworld.?cup\b|\bchampions.?league\b|\bpremier.?league\b|\bnba\b|\bnfl\b|",
        r"\bnhl\b|\bmlb\b|\bgrammy\b|\boscar\b|\bemmy\b|\beurovision\b"
    ))
    .unwrap()
});

/// Names and demonyms of a South Asian country in a headline.
fn aliases(country: &str) -> Vec<&str> {
    match country.to_lowercase().as_str() {
        "bangladesh" => vec!["Bangladesh", "Bangladeshi"],
        "india" => vec!["India", "Indian"],
        "pakistan" => vec!["Pakistan", "Pakistani"],
        "sri lanka" => vec!["Sri Lanka", "Sri Lankan", "Ceylon", "Ceylonese"],
        "myanmar" => vec!["Myanmar", "Burmese", "Burma"],
        "nepal" => vec!["Nepal", "Nepali", "Nepalese"],
        "bhutan" => vec!["Bhutan", "Bhutanese"],
        "maldives" => vec!["Maldives", "Maldivian"],
        _ => vec![country],
    }
}

pub fn relevant(title: &str, category: Option<&str>, unfiltered: bool, K: &Regex) -> bool {
    !NOISE.is_match(title)
        && (unfiltered || K.is_match(title) || K.is_match(category.unwrap_or("")))
}

/// South Asia Google News needs an in-title geographic match; country feeds also
/// need their own country or demonym in the headline. `countries` is the region's
/// configured list of {country, query} records.
pub fn regional_google_news(title: &str, sid: &str, K: &Regex, countries: &Value) -> bool {
    if NOISE.is_match(title) || REGIONAL_NOISE.is_match(title) || !K.is_match(title) {
        return false;
    }
    let Some(name) = sid.strip_prefix("gnews:country#") else {
        return true;
    };
    let name = name.to_lowercase();
    let Some(country) = countries
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x["country"].as_str())
        .find(|c| c.to_lowercase() == name)
    else {
        return false;
    };
    let p = aliases(country)
        .iter()
        .map(|a| regex::escape(a))
        .collect::<Vec<_>>()
        .join("|");
    // (?<!\w)(?:...)(?!\w): every alias begins and ends with a word character.
    Regex::new(&format!(r"(?i)\b(?:{p})\b"))
        .unwrap()
        .is_match(title)
}

pub fn derive(raws: &[Report]) -> Vec<Obs> {
    let mut out = vec![];
    for x in raws {
        let (Some(r), p) = (regions::get(&x.region), &x.payload) else {
            continue;
        };
        let title = p["title"].as_str().unwrap_or("");
        let gnews = x.source_id.starts_with("gnews:");
        let unfiltered = gnews
            || rss::feeds(r)
                .iter()
                .any(|(sid, f)| *sid == x.source_id && f["unfiltered"] == true);
        let K = r.regex("newsRelevanceKeywords");
        let accepted = if r.c("filterGoogleNews") == &Value::Bool(true) && gnews {
            regional_google_news(title, &x.source_id, &K, r.c("countryQueries"))
        } else {
            relevant(title, p["category"].as_str(), unfiltered, &K)
        };
        if accepted {
            out.push((r, x, title.to_string()));
        }
    }
    observations(
        &out.iter()
            .map(|(r, x, t)| (*r as &Region, *x, t.clone()))
            .collect::<Vec<_>>(),
    )
}
