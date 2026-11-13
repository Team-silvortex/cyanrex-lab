use super::*;

#[test]
fn accepts_both_explicit_timeout_endpoints() {
    let ad = announcement(1100);
    for timeout in [seconds(2), seconds(5)] {
        let mut plan = ad.initial_schedule(seconds(1000), timeout).unwrap();
        assert_eq!(
            plan.step(Duration::ZERO, seconds(1000), false),
            Ok(AnnouncementStep::Prepare { ordinal: 1 })
        );
    }
}

#[test]
fn rejects_timeout_below_two_seconds_or_above_five_without_a_plan() {
    let ad = announcement(1100);
    for timeout in [
        Duration::ZERO,
        seconds(2) - Duration::from_nanos(1),
        seconds(5) + Duration::from_nanos(1),
        Duration::MAX,
    ] {
        assert!(matches!(
            ad.initial_schedule(seconds(1000), timeout),
            Err(MdnsError::InvalidConfig)
        ));
    }
}

#[test]
fn rejects_a_new_origin_before_frozen_preparation_time() {
    let ad = announcement(1100);
    assert!(matches!(
        ad.initial_schedule(seconds(999), seconds(2)),
        Err(MdnsError::ClockInvalid)
    ));
}

#[test]
fn rejects_an_expired_declaration_at_construction() {
    let ad = announcement(1001);
    assert!(matches!(
        ad.initial_schedule(seconds(1001), seconds(2)),
        Err(MdnsError::RejectedAdvertisement(_))
    ));
}

#[test]
fn unrepresentable_wall_sample_does_not_create_a_live_schedule() {
    let ad = announcement(1100);
    assert!(ad.initial_schedule(Duration::MAX, seconds(2)).is_err());
}

#[test]
fn equal_subsecond_samples_are_valid_and_do_not_issue_duplicate_intents() {
    let ad = announcement(1100);
    let start = Duration::new(1000, 999_999_999);
    let mut plan = ad.initial_schedule(start, seconds(2)).unwrap();
    plan.step(Duration::ZERO, start, false).unwrap();
    plan.record_completion(Duration::ZERO, start, false)
        .unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, start, false),
        Ok(AnnouncementStep::Wait(seconds(1)))
    );
    assert_eq!(
        plan.step(Duration::ZERO, start, false),
        Ok(AnnouncementStep::Wait(seconds(1)))
    );
}

#[test]
fn wall_sample_rollback_by_one_nanosecond_in_step_is_terminal() {
    let ad = announcement(1100);
    let start = Duration::new(1000, 1);
    let mut plan = ad.initial_schedule(start, seconds(2)).unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(Duration::ZERO, start, false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn elapsed_rollback_from_a_wait_is_terminal() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(millis(500), seconds(1000), false).unwrap();
    assert_eq!(
        plan.step(millis(499), seconds(1000), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn elapsed_rollback_in_completion_is_terminal() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(millis(100), seconds(1000), false).unwrap();
    assert_eq!(
        plan.record_completion(millis(99), seconds(1000), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.record_completion(millis(100), seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn wall_rollback_in_completion_is_terminal() {
    let ad = announcement(1100);
    let start = Duration::new(1000, 100);
    let mut plan = ad.initial_schedule(start, seconds(2)).unwrap();
    plan.step(Duration::ZERO, start, false).unwrap();
    assert_eq!(
        plan.record_completion(Duration::ZERO, start - Duration::from_nanos(1), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(Duration::ZERO, start, false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn step_at_exact_timeout_fails_without_an_intent() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    assert_eq!(
        plan.step(seconds(2), seconds(1000), false),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn completion_at_exact_timeout_cannot_be_recorded() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    assert_eq!(
        plan.record_completion(seconds(2), seconds(1002), false),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1002), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn wait_is_capped_at_original_timeout_when_completion_is_late() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(millis(1500), seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(millis(1500), seconds(1001), false),
        Ok(AnnouncementStep::Wait(millis(500)))
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1002), false),
        Err(MdnsError::DeadlineExceeded)
    );
}

#[test]
fn wait_keeps_exact_nanosecond_precision_before_second_due() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(1) - Duration::from_nanos(1), seconds(1000), false),
        Ok(AnnouncementStep::Wait(Duration::from_nanos(1)))
    );
}

#[test]
fn short_fractional_f0_lease_is_not_rounded_up_to_one_second() {
    let ad = announcement(1001);
    let start = Duration::new(1000, 999_999_999);
    let mut plan = ad.initial_schedule(start, seconds(2)).unwrap();
    plan.step(Duration::ZERO, start, false).unwrap();
    plan.record_completion(Duration::ZERO, start, false)
        .unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, start, false),
        Ok(AnnouncementStep::Wait(Duration::from_nanos(1)))
    );
    assert_eq!(
        plan.step(Duration::from_nanos(1), start, false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(Duration::from_nanos(1), start, false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn stopped_wall_clock_cannot_extend_the_initial_monotonic_f0_lease() {
    let ad = announcement(1002);
    let start = Duration::new(1000, 750_000_000);
    let mut plan = ad.initial_schedule(start, seconds(5)).unwrap();
    plan.step(Duration::ZERO, start, false).unwrap();
    plan.record_completion(millis(500), start, false).unwrap();
    assert_eq!(
        plan.step(seconds(1), start, false),
        Ok(AnnouncementStep::Wait(millis(250)))
    );
    assert_eq!(
        plan.step(millis(1250), start, false),
        Err(MdnsError::ClockInvalid)
    );
}

#[test]
fn current_forward_wall_sample_caps_wait_before_frozen_monotonic_lease() {
    let ad = announcement(1002);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    let now = Duration::new(1001, 900_000_000);
    assert_eq!(
        plan.step(millis(100), now, false),
        Ok(AnnouncementStep::Wait(millis(100)))
    );
    assert_eq!(
        plan.step(millis(200), seconds(1002), false),
        Err(MdnsError::ClockInvalid)
    );
}

#[test]
fn wall_jump_to_expiry_is_terminal_even_when_elapsed_is_zero() {
    let ad = announcement(1001);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1001), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn second_completion_does_not_renew_frozen_expiry() {
    let ad = announcement(1002);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(seconds(1), seconds(1001), false).unwrap();
    plan.record_completion(seconds(1), seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(2), seconds(1001), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn a_huge_elapsed_sample_closes_without_overflow_or_catchup() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    assert_eq!(
        plan.step(Duration::MAX, seconds(1000), false),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn completion_at_wall_expiry_is_not_a_completed_model_action() {
    let ad = announcement(1002);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    assert_eq!(
        plan.record_completion(millis(100), seconds(1002), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(millis(100), seconds(1002), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn completion_with_a_stopped_wall_still_expires_the_original_monotonic_lease() {
    let ad = announcement(1001);
    let start = Duration::new(1000, 500_000_000);
    let mut plan = ad.initial_schedule(start, seconds(5)).unwrap();
    plan.step(Duration::ZERO, start, false).unwrap();
    assert_eq!(
        plan.record_completion(millis(500), start, false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.record_completion(millis(500), start, false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn completing_two_actions_does_not_bypass_the_original_timeout() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(seconds(1), seconds(1001), false).unwrap();
    plan.record_completion(seconds(1), seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(2), seconds(1002), false),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1002), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn forward_wall_watermark_cannot_roll_back_to_the_initial_wall() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    plan.step(Duration::ZERO, seconds(1001), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(millis(1), seconds(1000), false),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(
        plan.step(millis(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn last_nanosecond_before_timeout_is_not_a_rounded_up_wait() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(millis(1500), seconds(1001), false)
        .unwrap();
    let last = seconds(2) - Duration::from_nanos(1);
    assert_eq!(
        plan.step(last, seconds(1001), false),
        Ok(AnnouncementStep::Wait(Duration::from_nanos(1)))
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1001), false),
        Err(MdnsError::DeadlineExceeded)
    );
}

#[test]
fn second_completion_at_timeout_is_not_promoted_to_complete() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(seconds(1), seconds(1001), false).unwrap();
    assert_eq!(
        plan.record_completion(seconds(2), seconds(1002), false),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(
        plan.step(seconds(2), seconds(1002), false),
        Err(MdnsError::Closed)
    );
}
