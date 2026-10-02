use xhup_generator::generate_rime_artifacts;

#[test]
fn package_binds_source_policy_as_an_owned_hashed_artifact() {
    let package = generate_rime_artifacts();
    let policy = package
        .iter()
        .find(|a| a.filename() == "xhup_flow.sources.tsv")
        .unwrap();
    assert!(
        policy
            .contents()
            .starts_with("# distribution-policy=clean-v1\n")
    );
    assert!(policy.contents().contains("sogou-cell-research"));
    assert!(policy.contents().contains("redistribution-not-authorized"));
    assert!(policy.contents().contains("research-only"));
}
