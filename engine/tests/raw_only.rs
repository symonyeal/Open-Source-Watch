//! `osw collect --raw-only` (cycle::RAW_ONLY): raw reports and run rows are stored, the
//! observations they derive are not, and `osw rebuild` regenerates them. Its own test
//! binary, since RAW_ONLY is process-wide.
#![allow(non_snake_case)]

mod common;

use std::sync::atomic::Ordering;

use common::*;
use osw::schema::{Obs, Raw, Run};
use osw::{cycle, store};

#[test]
fn a_raw_only_poll_stores_raw_reports_and_runs_only() {
    let _g = fresh("raw_only");
    let s = sources_of("firms:VIIRS").remove(0);
    cycle::RAW_ONLY.store(true, Ordering::Relaxed);
    assert_eq!(ingest(&s, &text("firms.csv")).status, "ok");
    cycle::RAW_ONLY.store(false, Ordering::Relaxed);
    assert!(!store::rows::<Raw>().is_empty());
    assert_eq!(store::rows::<Run>().len(), 1);
    assert!(store::rows::<Obs>().is_empty());
    cycle::rebuild();
    assert!(!store::rows::<Obs>().is_empty());
}
