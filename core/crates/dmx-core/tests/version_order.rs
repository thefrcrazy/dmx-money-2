use dmx_core::settings::compare_versions;
use std::cmp::Ordering;

#[test]
fn stable_supersedes_release_candidates() {
    assert_eq!(compare_versions("2.0.3", "2.0.3-rc.2"), Ordering::Greater);
    assert_eq!(compare_versions("2.0.3-rc.2", "2.0.3"), Ordering::Less);
    assert_eq!(compare_versions("2.0.3-rc.10", "2.0.3-rc.2"), Ordering::Greater);
    assert_eq!(compare_versions("v2.0.3+build.1", "2.0.3"), Ordering::Equal);
}
