use super::*;
use chrono::{DateTime, Duration, Utc};

// Public RFC test material, not an account's enrollment secret.
const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const SECRET_BYTES: &[u8] = b"12345678901234567890";

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).unwrap()
}

fn code(counter: u64) -> String {
    super::super::super::hotp_code(SECRET_BYTES, counter)
}

#[test]
fn sha1_rfc_vectors_work_with_the_existing_six_digit_adaptation() {
    // RFC 6238 Appendix B's SHA1 vectors, reduced to the final six digits used by
    // this product. This is not a claim of full RFC conformance or eight-digit support.
    for (seconds, expected) in [
        (59, "287082"),
        (1_111_111_109, "081804"),
        (1_111_111_111, "050471"),
        (1_234_567_890, "005924"),
        (2_000_000_000, "279037"),
        (20_000_000_000, "353130"),
    ] {
        assert!(verify_at(SECRET, expected, at(seconds)), "{seconds}");
    }
}

#[test]
fn previous_current_and_next_thirty_second_steps_remain_accepted() {
    for counter in [9, 10, 11] {
        assert!(verify_at(SECRET, &code(counter), at(300)), "{counter}");
    }
}

#[test]
fn a_codes_exact_first_and_last_accepted_instants_follow_counter_boundaries() {
    let otp = code(10);
    assert!(!verify_at(SECRET, &otp, at(270) - Duration::nanoseconds(1)));
    assert!(verify_at(SECRET, &otp, at(270)));
    assert!(verify_at(SECRET, &otp, at(300) - Duration::nanoseconds(1)));
    assert!(verify_at(SECRET, &otp, at(300)));
    assert!(verify_at(SECRET, &otp, at(330)));
    assert!(verify_at(SECRET, &otp, at(360) - Duration::nanoseconds(1)));
    assert!(!verify_at(SECRET, &otp, at(360)));
}

#[test]
fn counters_beyond_the_one_step_window_are_rejected() {
    for counter in [0, 7, 8, 12, 13, 100] {
        assert!(!verify_at(SECRET, &code(counter), at(300)), "{counter}");
    }
}

#[test]
fn only_six_ascii_digits_are_accepted_after_trimming_outer_whitespace() {
    let otp = code(10);
    for candidate in [
        otp.clone(),
        format!(" \t{otp}\r\n"),
        format!("\u{2003}{otp}\u{00a0}"),
    ] {
        assert!(verify_at(SECRET, &candidate, at(300)));
    }
    for candidate in [
        "",
        " ",
        "12345",
        "1234567",
        "１２３４５６",
        "١٢٣٤٥٦",
        "123 45",
        "12\n345",
        "+12345",
        "-12345",
        "12345\0",
        "abcdef",
    ] {
        assert!(!verify_at(SECRET, candidate, at(300)), "{candidate:?}");
    }
}

#[test]
fn malformed_and_empty_secrets_fail_closed_even_for_the_empty_key_code() {
    let empty_key_code = super::super::super::hotp_code(&[], 10);
    for secret in ["", " \t\n", "\u{2003}", "!!!!", "A", "0", "1", "密码"] {
        assert!(!verify_at(secret, &empty_key_code, at(300)), "{secret:?}");
        assert!(!verify_at(secret, &code(10), at(300)), "{secret:?}");
    }
}

#[test]
fn existing_base32_case_and_whitespace_normalization_is_preserved() {
    let lowercase = SECRET.to_ascii_lowercase();
    let spaced = format!(" \t{}\n{} ", &lowercase[..16], &lowercase[16..]);
    assert!(verify_at(&lowercase, &code(10), at(300)));
    assert!(verify_at(&spaced, &code(10), at(300)));
}

#[test]
fn all_negative_timestamps_fail_closed_without_wrapping_a_counter() {
    for seconds in [-61, -30, -1] {
        for otp in [code(0), code(u64::MAX), code(u64::MAX - 1)] {
            assert!(!verify_at(SECRET, &otp, at(seconds)));
        }
    }
    assert!(!verify_at(
        SECRET,
        &code(0),
        at(0) - Duration::nanoseconds(1)
    ));
}

#[test]
fn the_epoch_skips_the_nonexistent_previous_counter() {
    assert!(verify_at(SECRET, &code(0), at(0)));
    assert!(verify_at(SECRET, &code(1), at(0)));
    assert!(!verify_at(SECRET, &code(u64::MAX), at(0)));
    assert!(!verify_at(SECRET, &code(2), at(0)));
}

#[test]
fn counters_larger_than_thirty_two_bits_do_not_truncate() {
    let counter = u64::from(u32::MAX) + 17;
    let instant = at(i64::try_from(counter * 30).unwrap());
    assert!(verify_at(SECRET, &code(counter), instant));
    assert!(verify_at(SECRET, &code(counter - 1), instant));
    assert!(verify_at(SECRET, &code(counter + 1), instant));
    assert!(!verify_at(SECRET, &code(counter as u32 as u64), instant));
}

#[test]
fn a_valid_code_is_reusable_within_its_window_not_consumed_by_verification() {
    let otp = code(10);
    for _ in 0..3 {
        assert!(verify_at(SECRET, &otp, at(300)));
    }
    assert!(verify_at(SECRET, &otp, at(359)));
    assert!(!verify_at(SECRET, &otp, at(360)));
}
