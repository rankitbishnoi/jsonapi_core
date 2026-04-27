#![cfg(feature = "derive")]

#[test]
fn compile_fail_tests() {
    // Snapshots are pinned to current stable rustc diagnostics; CI sets this on MSRV.
    if std::env::var_os("SKIP_TRYBUILD").is_some() {
        return;
    }
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
