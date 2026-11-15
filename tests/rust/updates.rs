use super::*;
#[test]
fn release_selection_uses_semantic_versions_and_repository_bound_urls() {
    let policy = Policy {
        repository: "Example/Project".into(),
        installed: "0.1.0-alpha.9".into(),
        channel: "alpha".into(),
        interval: 300.,
        enabled: true,
    };
    let rows = vec![
        json!({"tag_name":"v0.1.0-alpha.10","draft":false,"published_at":"fixture","html_url":"https://untrusted.invalid/","assets":[{"name":"upgrade.json"}]}),
        json!({"tag_name":"v9.0.0","draft":true,"published_at":"fixture"}),
        json!({"tag_name":"garbage","published_at":"fixture"}),
    ];
    let latest = select(&rows, &policy).unwrap();
    assert_eq!(latest["tag"], "v0.1.0-alpha.10");
    assert_eq!(
        latest["url"],
        "https://github.com/Example/Project/releases/tag/v0.1.0-alpha.10"
    );
    assert!(latest["upgrade"]["descriptor_url"]
        .as_str()
        .unwrap()
        .starts_with("https://github.com/Example/Project/"));
}
#[test]
fn stable_release_discovery_rejects_prereleases_and_invalid_identifiers() {
    let policy = Policy {
        repository: "Example/Project".into(),
        installed: "1.0.0".into(),
        channel: "stable".into(),
        interval: 300.,
        enabled: true,
    };
    assert!(version("01.0.0").is_none());
    assert!(version("1.0.0-alpha.01").is_none());
    assert!(version("1.0.0/escape").is_none());
    assert!(version("1.0.0+../../escape").is_none());
    assert!(select(
        &[json!({"tag_name":"2.0.0-alpha.1","published_at":"fixture","prerelease":true})],
        &policy
    )
    .is_none());
}
