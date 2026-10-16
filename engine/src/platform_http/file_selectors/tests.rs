use super::{decimal, parse, MAX_QUERY_BYTES};

#[test]
fn raw_selectors_are_borrowed_order_independent_and_require_each_exact_slot_once() {
    let names = ["grant_id", "file_id", "sha256"];
    let query = "sha256=c&grant_id=a&file_id=b";
    let values = parse(Some(query), names).unwrap_or_else(|_| panic!("exact raw inventory"));
    assert_eq!(values, ["a", "b", "c"]);
    for value in values {
        assert!(
            (query.as_ptr() as usize..query.as_ptr() as usize + query.len())
                .contains(&(value.as_ptr() as usize))
        );
    }
    assert!(parse(None, names).is_err());
    for invalid in [
        "",
        "grant_id=a&file_id=b",
        "grant_id=a&sha256=c",
        "file_id=b&sha256=c",
        "grant_id=a&file_id=b&sha256=c&grant_id=a",
        "grant_id=a&file_id=b&sha256=c&grant_id=d",
        "grant_id=a&file_id=b&sha256=c&chunk_index=0",
        "Grant_id=a&file_id=b&sha256=c",
        "grant_id=&file_id=b&sha256=c",
        "grant_id=a=b&file_id=b&sha256=c",
        "grant_id&file_id=b&sha256=c",
        "grant_id=a&&file_id=b&sha256=c",
        "&grant_id=a&file_id=b&sha256=c",
        "grant_id=a&file_id=b&sha256=c&",
        "grant_id=%61&file_id=b&sha256=c",
        "grant_id=a&file_id=b&sha256=+c",
    ] {
        assert!(parse(Some(invalid), names).is_err(), "{invalid}");
    }
}

#[test]
fn raw_selector_budget_accepts_exactly_512_utf8_bytes_before_domain_validation() {
    for value in ["a".repeat(504), "界".repeat(168)] {
        // These are valid raw values only, deliberately not canonical typed file IDs.
        let query = format!("file_id={value}");
        assert_eq!(query.len(), MAX_QUERY_BYTES);
        assert_eq!(
            parse(Some(&query), ["file_id"]).ok(),
            Some([value.as_str()])
        );
        assert!(parse(Some(&format!("{query}a")), ["file_id"]).is_err());
    }
}

#[test]
fn selector_decimal_preserves_zero_u64_max_and_rejects_noncanonical_aliases() {
    assert_eq!(decimal("0").ok(), Some(0));
    assert_eq!(decimal("18446744073709551615").ok(), Some(u64::MAX));
    for value in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e1",
        " 1",
        "1 ",
        "1\n",
        "１",
        "١",
        "18446744073709551616",
        "1\0",
    ] {
        assert!(decimal(value).is_err(), "{value:?}");
    }
}
