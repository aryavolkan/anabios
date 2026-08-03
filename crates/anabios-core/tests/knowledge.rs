use anabios_core::scenario::Scenario;

#[test]
fn knowledge_flag_requires_inventions() {
    let bad = "name=\"k\"\nseed=1\nworld_size=64\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n";
    assert!(Scenario::parse_toml(bad).is_err());
    let ok = "name=\"k\"\nseed=1\nworld_size=64\ninventions_enabled=true\nknowledge_enabled=true\n[[agents]]\narchetype=\"grazer\"\ncount=4\n";
    let w = Scenario::parse_toml(ok).unwrap().instantiate();
    assert!(w.knowledge_enabled);
}
