//! Text forms of times and numbers. The API and the stored payloads keep the forms
//! the engine has always written (Python's `str`, `isoformat`, `repr` and `round`),
//! so the front end, the `data` branch and the products read the same.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc};

pub type Time = DateTime<Utc>;

/// `YYYY-MM-DD HH:MM:SS[.ffffff]+00:00`, the form of a time in API rows.
pub fn py(t: &Time) -> String {
    stamp(t, ' ')
}

/// `YYYY-MM-DDTHH:MM:SS[.ffffff]+00:00`, ISO 8601.
pub fn iso(t: &Time) -> String {
    stamp(t, 'T')
}

fn stamp(t: &Time, sep: char) -> String {
    let us = t.nanosecond() / 1000;
    let f = if us > 0 {
        format!(".{us:06}")
    } else {
        String::new()
    };
    format!(
        "{}{sep}{}{f}+00:00",
        t.format("%Y-%m-%d"),
        t.format("%H:%M:%S")
    )
}

/// Serde forms of times: `py` in rows, `iso` in views.
pub mod ser {
    use super::Time;
    use serde::Serializer;

    pub fn py<S: Serializer>(t: &Time, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::py(t))
    }

    pub fn py_opt<S: Serializer>(t: &Option<Time>, s: S) -> Result<S::Ok, S::Error> {
        match t {
            Some(t) => s.serialize_str(&super::py(t)),
            None => s.serialize_none(),
        }
    }
}

/// Shortest round-trip text of x, with Python's layout: fixed notation for
/// 1e-4 <= |x| < 1e16 (an integral value keeps ".0"), else `d.ddde±XX`.
pub fn repr(x: f64) -> String {
    if !x.is_finite() {
        return if x.is_nan() {
            "nan".into()
        } else if x > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let s = format!("{x:e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let (sign, m) = m.strip_prefix('-').map_or(("", m), |m| ("-", m));
    let d: String = m.chars().filter(|c| *c != '.').collect();
    if (-4..16).contains(&e) {
        let k = e + 1; // digits before the point
        let body = if k <= 0 {
            format!("0.{}{d}", "0".repeat((-k) as usize))
        } else if (k as usize) >= d.len() {
            format!("{d}{}.0", "0".repeat(k as usize - d.len()))
        } else {
            format!("{}.{}", &d[..k as usize], &d[k as usize..])
        };
        format!("{sign}{body}")
    } else {
        let m = if d.len() > 1 {
            format!("{}.{}", &d[..1], &d[1..])
        } else {
            d
        };
        format!("{sign}{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    }
}

/// x rounded to n decimals, correctly rounded as Python's `round(x, n)`.
pub fn round(x: f64, n: usize) -> f64 {
    format!("{x:.n$}").parse().unwrap()
}

/// Time of an ISO 8601 text as Python's `datetime.fromisoformat` reads it: a date,
/// or a date and time (separator `T` or space, optional fraction), with an offset
/// `Z`, `±HH:MM[:SS]` or `±HHMM`; a time without offset is taken as UTC.
pub fn fromiso(s: &str) -> Option<Time> {
    let s = s.trim();
    if s.len() == 10 {
        let d = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
        return Some(Utc.from_utc_datetime(&d.and_time(NaiveTime::MIN)));
    }
    let (date, rest) = (s.get(..10)?, s.get(11..)?);
    if !matches!(s.as_bytes().get(10), Some(b'T' | b' ')) {
        return None;
    }
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let k = rest.find(['+', '-', 'Z']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(k);
    let t = clock_of(clock)?;
    let off = match zone {
        "" => 0,
        "Z" => 0,
        z => {
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let z: String = z[1..].chars().filter(|c| *c != ':').collect();
            let (h, m, s) = (
                z.get(..2)?,
                z.get(2..4).unwrap_or("00"),
                z.get(4..6).unwrap_or("00"),
            );
            sign * (h.parse::<i64>().ok()? * 3600
                + m.parse::<i64>().ok()? * 60
                + s.parse::<i64>().ok()?)
        }
    };
    let n = NaiveDateTime::new(d, t) - chrono::Duration::seconds(off);
    Some(Utc.from_utc_datetime(&n))
}

fn clock_of(s: &str) -> Option<NaiveTime> {
    let (hms, frac) = s.split_once(['.', ',']).unwrap_or((s, ""));
    let p: Vec<&str> = if hms.contains(':') {
        hms.split(':').collect()
    } else {
        (0..hms.len())
            .step_by(2)
            .map(|i| hms.get(i..i + 2).unwrap_or(""))
            .collect()
    };
    let n = |k: usize| p.get(k).map_or(Some(0), |x| x.parse::<u32>().ok());
    let us = if frac.is_empty() {
        0
    } else {
        format!("{:0<6}", &frac[..frac.len().min(6)]).parse().ok()?
    };
    NaiveTime::from_hms_micro_opt(n(0)?, n(1)?, n(2)?, us)
}

/// Time of epoch seconds x, rounded to the microsecond half to even.
pub fn epoch(x: f64) -> Time {
    let s = x.floor();
    let us = ((x - s) * 1e6).round_ties_even() as i64;
    Utc.timestamp_micros(s as i64 * 1_000_000 + us).unwrap()
}

/// JSON text as Python's `json.dumps` writes it: separators ", " and ": " (one item
/// per line under `indent`), floats by `repr`, keys sorted under `sort`, non-ASCII
/// escaped under `ascii`.
pub fn dumps(v: &serde_json::Value, indent: Option<usize>, sort: bool, ascii: bool) -> String {
    let mut s = String::new();
    put(&mut s, v, indent, 0, sort, ascii);
    s
}

fn put(
    s: &mut String,
    v: &serde_json::Value,
    ind: Option<usize>,
    depth: usize,
    sort: bool,
    ascii: bool,
) {
    use serde_json::Value::*;
    let nl = |s: &mut std::string::String, d: usize| {
        if let Some(k) = ind {
            s.push('\n');
            s.push_str(&" ".repeat(k * d));
        }
    };
    let sep = if ind.is_some() { "," } else { ", " };
    match v {
        Null => s.push_str("null"),
        Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Number(n) => match n.as_f64().filter(|_| n.is_f64()) {
            Some(x) if x.is_nan() => s.push_str("NaN"),
            Some(x) if x.is_infinite() => {
                s.push_str(if x > 0.0 { "Infinity" } else { "-Infinity" })
            }
            Some(x) => s.push_str(&repr(x)),
            None => s.push_str(&n.to_string()),
        },
        String(t) => quote(s, t, ascii),
        Array(a) if a.is_empty() => s.push_str("[]"),
        Array(a) => {
            s.push('[');
            for (k, x) in a.iter().enumerate() {
                if k > 0 {
                    s.push_str(sep);
                }
                nl(s, depth + 1);
                put(s, x, ind, depth + 1, sort, ascii);
            }
            nl(s, depth);
            s.push(']');
        }
        Object(m) if m.is_empty() => s.push_str("{}"),
        Object(m) => {
            let mut K: Vec<_> = m.iter().collect();
            if sort {
                K.sort_by(|a, b| a.0.cmp(b.0));
            }
            s.push('{');
            for (k, (key, x)) in K.into_iter().enumerate() {
                if k > 0 {
                    s.push_str(sep);
                }
                nl(s, depth + 1);
                quote(s, key, ascii);
                s.push_str(": ");
                put(s, x, ind, depth + 1, sort, ascii);
            }
            nl(s, depth);
            s.push('}');
        }
    }
}

/// A JSON string; Python escapes `"`, `\`, control characters and, under
/// `ascii`, everything outside space..`~` (as UTF-16 units).
fn quote(s: &mut String, t: &str, ascii: bool) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            c if (c as u32) < 0x20 || (ascii && c > '~') => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    s.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_json_text() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"b": [1, 2.0, "é\n"], "a": {}}"#).unwrap();
        assert_eq!(
            dumps(&v, None, false, false),
            "{\"b\": [1, 2.0, \"é\\n\"], \"a\": {}}"
        );
        assert_eq!(
            dumps(&v, None, true, true),
            "{\"a\": {}, \"b\": [1, 2.0, \"\\u00e9\\n\"]}"
        );
        assert_eq!(
            dumps(&v, Some(2), false, false),
            "{\n  \"b\": [\n    1,\n    2.0,\n    \"é\\n\"\n  ],\n  \"a\": {}\n}"
        );
    }

    #[test]
    fn python_float_repr() {
        for (x, s) in [
            (30.0, "30.0"),
            (0.1, "0.1"),
            (1e-5, "1e-05"),
            (1.5e16, "1.5e+16"),
            (-2.25, "-2.25"),
            (123456.789, "123456.789"),
            (0.0001, "0.0001"),
        ] {
            assert_eq!(repr(x), s);
        }
    }

    #[test]
    fn iso_texts() {
        let t = fromiso("2026-10-21T08:00:01.5+03:00").unwrap();
        assert_eq!(iso(&t), "2026-10-21T05:00:01.500000+00:00");
        assert_eq!(
            py(&fromiso("2026-10-21").unwrap()),
            "2026-10-21 00:00:00+00:00"
        );
        assert!(fromiso("yesterday").is_none());
    }
}
