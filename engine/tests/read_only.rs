//! `osw serve --no-collect` (cycle::READ_ONLY): a poll is parsed and returned but the
//! store is not written. Its own test binary, since READ_ONLY is process-wide.
#![allow(non_snake_case)]

mod common;

use std::sync::atomic::Ordering;

use common::*;
use osw::schema::{Obs, Raw, Run};
use osw::{cycle, store};

#[test]
fn a_read_only_poll_stores_nothing() {
    let _g = fresh("read_only");
    let s = sources_of("rss:BBC").remove(0);
    cycle::READ_ONLY.store(true, Ordering::Relaxed);
    assert_eq!(ingest(&s, &text("bbc_rss.xml")).status, "ok");
    let n = (
        store::rows::<Raw>().len(),
        store::rows::<Obs>().len(),
        store::rows::<Run>().len(),
    );
    cycle::READ_ONLY.store(false, Ordering::Relaxed);
    assert_eq!(n, (0, 0, 0));
    ingest(&s, &text("bbc_rss.xml"));
    assert!(!store::rows::<Raw>().is_empty());
}
