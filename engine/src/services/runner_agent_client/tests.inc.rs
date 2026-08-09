#[test]
fn engine_url_requires_tls_outside_loopback() {
    assert_eq!(
        validate_engine_url("http://127.0.0.1:8080", false).unwrap(),
        "http://127.0.0.1:8080"
    );
    assert!(validate_engine_url("http://192.0.2.10:8080", false).is_err());
    assert!(validate_engine_url("http://192.0.2.10:8080", true).is_ok());
    assert!(validate_engine_url("https://engine.lab.example", false).is_ok());
}

#[test]
fn engine_url_rejects_redirectable_or_ambiguous_components() {
    assert!(validate_engine_url("https://user@engine.lab.example", false).is_err());
    assert!(validate_engine_url("https://engine.lab.example/api", false).is_err());
    assert!(validate_engine_url("https://engine.lab.example?next=other", false).is_err());
    assert!(validate_engine_url("file:///tmp/engine.sock", false).is_err());
}
