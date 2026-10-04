use super::*;
use chrono::{DateTime, Duration, Utc};

// Public algorithm fixtures only, never a user's enrollment material.
const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const KEY: &[u8] = b"12345678901234567890";
// Found once using independent HMAC-SHA1 computation; no runtime collision search.
const COLLISION_LOW: i64 = 910_737;
const COLLISION_HIGH: i64 = 910_738;
const COLLISION_CODE: &str = "911617";
const OTHER_KEY: &[u8] = b"public-fixture-key-96927";

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).unwrap()
}

fn watermark(value: i64) -> Watermark {
    Watermark::from_stored(value).expect("valid test watermark")
}

fn code(counter: u64) -> String {
    super::super::super::hotp_code(KEY, counter)
}

fn rejected(secret: &str, otp: &str, instant: DateTime<Utc>, previous: i64) {
    assert_eq!(
        prepare(secret, otp, instant, watermark(previous)).err(),
        Some(ConsumptionError::InvalidCredentials)
    );
}

#[test]
fn stored_watermarks_accept_minus_one_and_every_nonnegative_i64_only() {
    for stored in [-1, 0, 1, i64::from(u32::MAX), i64::MAX - 1, i64::MAX] {
        assert_eq!(Watermark::from_stored(stored).unwrap().stored(), stored);
    }
    for stored in [i64::MIN, -1_000, -2] {
        assert_eq!(
            Watermark::from_stored(stored).err(),
            Some(ConsumptionError::InvalidState)
        );
    }
}

#[test]
fn public_rfc_sha1_vectors_propose_their_exact_six_digit_counter() {
    // Six-digit adaptation of RFC 6238 Appendix B, not full RFC conformance.
    for (seconds, otp) in [
        (59, "287082"),
        (1_111_111_109, "081804"),
        (1_111_111_111, "050471"),
        (1_234_567_890, "005924"),
        (2_000_000_000, "279037"),
        (20_000_000_000, "353130"),
    ] {
        let pending = prepare(SECRET, otp, at(seconds), watermark(-1)).unwrap();
        assert_eq!(pending.previous().stored(), -1);
        assert_eq!(pending.next().stored(), seconds / 30);
        assert_eq!(pending.recheck(SECRET, otp, at(seconds)), Ok(()));
    }
}

#[test]
fn previous_current_and_future_steps_are_proposed_only_when_above_the_watermark() {
    for counter in [9, 10, 11] {
        let otp = code(counter);
        let pending = prepare(SECRET, &otp, at(300), watermark(-1)).unwrap();
        assert_eq!(pending.previous().stored(), -1);
        assert_eq!(pending.next().stored(), counter as i64);
    }
    for counter in [8, 12] {
        rejected(SECRET, &code(counter), at(300), -1);
    }
    for previous in [10, 11, i64::MAX] {
        rejected(SECRET, &code(10), at(300), previous);
    }
    assert_eq!(
        prepare(SECRET, &code(11), at(300), watermark(10))
            .unwrap()
            .next()
            .stored(),
        11
    );
}

#[test]
fn accepting_a_future_step_also_closes_lower_unconsumed_steps() {
    let future = prepare(SECRET, &code(11), at(300), watermark(-1)).unwrap();
    assert_eq!(future.next().stored(), 11);
    rejected(SECRET, &code(9), at(300), future.next().stored());
    rejected(SECRET, &code(10), at(300), future.next().stored());
    rejected(SECRET, &code(11), at(330), future.next().stored());
    assert_eq!(
        prepare(SECRET, &code(12), at(330), future.next())
            .unwrap()
            .next()
            .stored(),
        12
    );
}

#[test]
fn fixed_adjacent_collision_selects_the_highest_matching_unconsumed_counter() {
    assert_eq!(code(COLLISION_LOW as u64), COLLISION_CODE);
    assert_eq!(code(COLLISION_HIGH as u64), COLLISION_CODE);
    assert_ne!(code((COLLISION_LOW - 1) as u64), COLLISION_CODE);
    assert_ne!(code((COLLISION_HIGH + 1) as u64), COLLISION_CODE);
    let pending = prepare(
        SECRET,
        COLLISION_CODE,
        at(COLLISION_LOW * 30),
        watermark(COLLISION_LOW - 1),
    )
    .unwrap();
    assert_eq!(pending.previous().stored(), COLLISION_LOW - 1);
    assert_eq!(pending.next().stored(), COLLISION_HIGH);
}

#[test]
fn any_consumed_collision_rejects_even_if_a_later_matching_step_is_unconsumed() {
    // Filtering out old candidates before matching would incorrectly accept HIGH here.
    for previous in [COLLISION_LOW, COLLISION_HIGH] {
        rejected(SECRET, COLLISION_CODE, at(COLLISION_LOW * 30), previous);
    }
}

#[test]
fn final_recheck_cannot_retarget_to_a_later_collision_while_the_original_still_matches() {
    let initial_time = at((COLLISION_LOW - 1) * 30);
    let pending = prepare(SECRET, COLLISION_CODE, initial_time, watermark(-1)).unwrap();
    assert_eq!(pending.next().stored(), COLLISION_LOW);
    assert_eq!(
        pending.recheck(SECRET, COLLISION_CODE, initial_time),
        Ok(())
    );
    let later_time = at(COLLISION_LOW * 30);
    assert_eq!(
        prepare(SECRET, COLLISION_CODE, later_time, watermark(-1))
            .unwrap()
            .next()
            .stored(),
        COLLISION_HIGH
    );
    assert_eq!(
        pending.recheck(SECRET, COLLISION_CODE, later_time),
        Err(ConsumptionError::InvalidCredentials)
    );
}

#[test]
fn final_recheck_applies_the_original_watermark_to_newly_visible_old_collisions() {
    // Initially only HIGH is in the window. Moving back reveals the already consumed LOW.
    let pending = prepare(
        SECRET,
        COLLISION_CODE,
        at((COLLISION_HIGH + 1) * 30),
        watermark(COLLISION_LOW),
    )
    .unwrap();
    assert_eq!(pending.next().stored(), COLLISION_HIGH);
    assert_eq!(
        pending.recheck(SECRET, COLLISION_CODE, at(COLLISION_LOW * 30)),
        Err(ConsumptionError::InvalidCredentials)
    );
}

#[test]
fn final_recheck_accepts_the_same_proposal_only_while_the_exact_counter_is_in_window() {
    let otp = code(10);
    let pending = prepare(SECRET, &otp, at(300), watermark(9)).unwrap();
    assert_eq!(pending.recheck(SECRET, &otp, at(270)), Ok(()));
    assert_eq!(
        pending.recheck(SECRET, &otp, at(360) - Duration::nanoseconds(1)),
        Ok(())
    );
    for instant in [at(270) - Duration::nanoseconds(1), at(360), at(-1)] {
        assert_eq!(
            pending.recheck(SECRET, &otp, instant),
            Err(ConsumptionError::InvalidCredentials)
        );
    }
    assert_eq!(pending.previous().stored(), 9);
    assert_eq!(pending.next().stored(), 10);
}

#[test]
fn recheck_binds_decoded_secret_even_when_another_key_has_the_same_code_and_counter() {
    let other_secret = data_encoding::BASE32.encode(OTHER_KEY);
    let otp = code(10);
    assert_eq!(otp, "403154");
    assert_eq!(super::super::super::hotp_code(OTHER_KEY, 10), otp);
    let original = prepare(SECRET, &otp, at(300), watermark(-1)).unwrap();
    let other = prepare(&other_secret, &otp, at(300), watermark(-1)).unwrap();
    assert_eq!(original.next().stored(), other.next().stored());
    assert_eq!(
        original.recheck(&other_secret, &otp, at(300)),
        Err(ConsumptionError::InvalidCredentials)
    );
}

#[test]
fn equivalent_secret_encodings_and_trimmed_codes_keep_the_same_binding() {
    let lower = SECRET.to_ascii_lowercase();
    let spaced = format!(" \t{}\n{} ", &lower[..16], &lower[16..]);
    let otp = code(10);
    let padded = format!("\u{2003}{otp}\u{00a0}");
    let pending = prepare(&spaced, &padded, at(300), watermark(-1)).unwrap();
    assert_eq!(pending.recheck(SECRET, &otp, at(300)), Ok(()));
    assert_eq!(pending.recheck(&lower, &padded, at(300)), Ok(()));
    assert_eq!(
        pending.recheck(SECRET, &code(11), at(300)),
        Err(ConsumptionError::InvalidCredentials)
    );
}

#[test]
fn raw_secret_and_code_limits_are_inclusive_byte_limits_before_normalization() {
    let secret = format!("{}{}", "\u{2003}".repeat(32), SECRET);
    let otp = format!("{} {}", "\u{2003}".repeat(19), code(10));
    assert_eq!(secret.len(), 128);
    assert_eq!(otp.len(), 64);
    let pending = prepare(&secret, &otp, at(300), watermark(-1)).unwrap();
    assert_eq!(pending.recheck(&secret, &otp, at(300)), Ok(()));
    assert_eq!(pending.recheck(SECRET, &code(10), at(300)), Ok(()));
    rejected(&format!(" {secret}"), &otp, at(300), -1);
    rejected(&secret, &format!(" {otp}"), at(300), -1);
    assert_eq!(
        pending.recheck(&format!(" {secret}"), &otp, at(300)),
        Err(ConsumptionError::InvalidCredentials)
    );
    assert_eq!(
        pending.recheck(&secret, &format!(" {otp}"), at(300)),
        Err(ConsumptionError::InvalidCredentials)
    );
}

#[test]
fn malformed_codes_or_invalid_empty_secrets_never_produce_a_proposal() {
    for otp in [
        "",
        "12345",
        "1234567",
        "１２３４５６",
        "١٢٣٤٥٦",
        "123 45",
        "12345\0",
    ] {
        rejected(SECRET, otp, at(300), -1);
    }
    let empty_key_code = super::super::super::hotp_code(&[], 10);
    for secret in ["", " \t\n", "0", "1", "A", "!!!!", "密码"] {
        rejected(secret, &empty_key_code, at(300), -1);
    }
}

#[test]
fn negative_times_fail_closed_and_epoch_never_matches_a_wrapped_previous_counter() {
    for seconds in [-61, -30, -1] {
        rejected(SECRET, &code(0), at(seconds), -1);
        rejected(SECRET, &code(u64::MAX), at(seconds), -1);
    }
    rejected(SECRET, &code(u64::MAX), at(0), -1);
    for counter in [0, 1] {
        assert_eq!(
            prepare(SECRET, &code(counter), at(0), watermark(-1))
                .unwrap()
                .next()
                .stored(),
            counter as i64
        );
    }
}

#[test]
fn large_counters_and_the_last_chrono_window_do_not_truncate_or_overflow() {
    let counter = i64::from(u32::MAX) + 17;
    let pending = prepare(
        SECRET,
        &code(counter as u64),
        at(counter * 30),
        watermark(counter - 1),
    )
    .unwrap();
    assert_eq!(pending.next().stored(), counter);
    let last_counter = DateTime::<Utc>::MAX_UTC.timestamp().div_euclid(30);
    let pending = prepare(
        SECRET,
        &code((last_counter + 1) as u64),
        DateTime::<Utc>::MAX_UTC,
        watermark(-1),
    )
    .unwrap();
    assert_eq!(pending.next().stored(), last_counter + 1);
    rejected(
        SECRET,
        &code((last_counter + 1) as u64),
        DateTime::<Utc>::MAX_UTC,
        i64::MAX,
    );
}

#[test]
fn proposals_are_pure_values_not_a_consumption_ledger_or_atomic_compare_and_set() {
    let otp = code(10);
    let original = 9;
    let concurrent = prepare(SECRET, &otp, at(300), watermark(original)).unwrap();
    {
        let first = prepare(SECRET, &otp, at(300), watermark(original)).unwrap();
        assert_eq!(first.previous().stored(), concurrent.previous().stored());
        assert_eq!(first.next().stored(), concurrent.next().stored());
    } // A caller abandoning a proposal has not changed any external state here.
    let retried = prepare(SECRET, &otp, at(300), watermark(original)).unwrap();
    assert_eq!(retried.next().stored(), 10);
    // Only supplying the advanced state rejects replay. A caller must persist/CAS this atomically;
    // the pure helper cannot notice another request's commit or establish rollback by itself.
    rejected(SECRET, &otp, at(300), retried.next().stored());
    assert_eq!(concurrent.recheck(SECRET, &otp, at(300)), Ok(()));
    assert_eq!(concurrent.previous().stored(), original);
}
