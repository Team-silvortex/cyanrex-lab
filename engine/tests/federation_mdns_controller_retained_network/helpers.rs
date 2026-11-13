// Reuse the retained fixture's exact FD, queue, provenance and peer-capture checks. The runner
// enables its fixture marker only together with the separate controller-retained opt-in below.
include!("../federation_mdns_retained_network/helpers.rs");

pub(super) fn controller_case(expected_udp: u64, case: impl FnOnce()) {
    assert_controller_opt_in();
    network_case(expected_udp, case);
}

pub(super) fn controller_mutation_case(expected_udp: u64, case: impl FnOnce()) {
    assert_controller_opt_in();
    network_mutation_case(expected_udp, case);
}

fn assert_controller_opt_in() {
    assert_eq!(
        std::env::var("CYANREX_MDNS_TEST_WITH_CONTROLLER_RETAINED").unwrap(),
        "1"
    );
}

pub(super) fn monotonic_ns() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: the fixed monotonic clock writes only this live, correctly sized timespec.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) },
        0
    );
    (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64
}

pub(super) fn finish_after(expected: usize, reference: &[u8], conflict_returned: u64) {
    // wait-probes consumes the capture. Validate its full existing contract in this one read,
    // then compare its first peer monotonic receive sample against the caller's post-conflict sample.
    let response = rpc(json!({"op":"wait-probes"}));
    let queries = response["queries"].as_array().unwrap();
    assert_eq!(
        queries.len(),
        expected,
        "unexpected later probe or announcement"
    );
    for query in queries {
        assert_eq!(query["source"], "10.23.0.1");
        assert_eq!(query["port"], 5353);
        assert_eq!(query["ttl"], 255);
        let bytes: Vec<u8> = serde_json::from_value(query["data"].clone()).unwrap();
        assert_eq!(canonical_probe(&bytes), canonical_probe(reference));
    }
    for pair in queries.windows(2) {
        let elapsed = pair[1]["ns"].as_u64().unwrap() - pair[0]["ns"].as_u64().unwrap();
        assert!(elapsed >= 240_000_000, "probes repeated too quickly");
    }
    let first = queries.first().unwrap()["ns"].as_u64().unwrap();
    assert!(
        first.checked_sub(conflict_returned).unwrap() >= 5_000_000_000,
        "the first retry packet preceded the full post-conflict cooldown"
    );
}
