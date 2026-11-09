//! `store::inputs` records the same bytes whatever line endings the checkout has.
//! Its own test binary: it points OSW_ROOT at a scratch repository, process-wide.

mod common;

use osw::store;

#[test]
fn line_endings_do_not_change_the_recorded_inputs() {
    let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-inputs");
    let _ = std::fs::remove_dir_all(&d);
    let labels = d.join("repo/docs/labels");
    std::fs::create_dir_all(&labels).unwrap();
    std::fs::create_dir_all(d.join("repo/regions")).unwrap();
    // SAFETY: set before any other thread of this binary reads it.
    unsafe { std::env::set_var("OSW_ROOT", d.join("repo")) };
    store::set_root(Some(d.join("data")));
    let n = |text: &str| {
        std::fs::write(labels.join("a.jsonl"), text).unwrap();
        store::transaction(store::inputs);
        std::fs::read_dir(d.join("data/.inputs")).unwrap().count()
    };
    assert_eq!(n("x\ny\n"), 1);
    assert_eq!(n("x\r\ny\r\n"), 1);
    assert_eq!(n("x\r\nz\n"), 2);
}
