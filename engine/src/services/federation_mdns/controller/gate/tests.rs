use super::*;

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

fn cooling_at(completed: u64) -> RetryGate {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    gate.finish(seconds(completed), true).unwrap();
    gate
}

#[test]
fn construction_is_ready_without_an_attempt_or_deadline_hint() {
    let gate = RetryGate::new();
    assert_eq!(gate.state(), ProbeControllerState::Ready);
    assert_eq!(gate.attempts_started(), 0);
    assert_eq!(gate.remaining(Duration::ZERO), None);
}

#[test]
fn beginning_at_origin_spends_one_attempt_without_exposing_an_inflight_permit() {
    let mut gate = RetryGate::new();
    assert_eq!(gate.begin(Duration::ZERO), Ok(()));
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.remaining(Duration::ZERO), None);
}

#[test]
fn first_begin_can_be_late_but_not_at_the_lifetime_boundary() {
    let mut gate = RetryGate::new();
    assert_eq!(gate.begin(seconds(29)), Ok(()));
    let mut expired = RetryGate::new();
    assert_eq!(expired.begin(seconds(30)), Err(MdnsError::DeadlineExceeded));
    assert_eq!(expired.state(), ProbeControllerState::Closed);
    assert_eq!(expired.attempts_started(), 0);
}

#[test]
fn the_nanosecond_before_lifetime_is_still_admissible() {
    let mut gate = RetryGate::new();
    assert_eq!(gate.begin(MAX_LIFETIME - Duration::from_nanos(1)), Ok(()));
}

#[test]
fn begin_far_after_lifetime_closes_without_consuming_an_attempt() {
    let mut gate = RetryGate::new();
    assert_eq!(gate.begin(Duration::MAX), Err(MdnsError::DeadlineExceeded));
    assert_eq!(gate.attempts_started(), 0);
    assert_eq!(gate.begin(Duration::ZERO), Err(MdnsError::Closed));
}

#[test]
fn a_second_begin_while_inflight_closes_permanently() {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    assert_eq!(gate.begin(seconds(1)), Err(MdnsError::Closed));
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.finish(seconds(1), false), Err(MdnsError::Closed));
    assert_eq!(gate.begin(seconds(5)), Err(MdnsError::Closed));
}

#[test]
fn finishing_before_begin_closes_without_creating_a_cooldown() {
    let mut gate = RetryGate::new();
    assert_eq!(gate.finish(Duration::ZERO, true), Err(MdnsError::Closed));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.attempts_started(), 0);
    assert_eq!(gate.remaining(Duration::ZERO), None);
}

#[test]
fn quiet_observation_is_terminal_without_a_reusable_permit() {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    assert_eq!(gate.finish(seconds(1), false), Ok(()));
    assert_eq!(gate.state(), ProbeControllerState::Finished);
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.remaining(seconds(1)), None);
    assert_eq!(gate.begin(seconds(6)), Err(MdnsError::Closed));
    assert_eq!(gate.finish(seconds(6), false), Err(MdnsError::Closed));
    assert_eq!(gate.state(), ProbeControllerState::Finished);
}

#[test]
fn terminal_checks_take_precedence_over_old_or_expired_time_samples() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(10)).unwrap();
    gate.finish(seconds(11), false).unwrap();
    for now in [Duration::ZERO, seconds(30), Duration::MAX] {
        assert_eq!(gate.begin(now), Err(MdnsError::Closed));
        assert_eq!(gate.finish(now, true), Err(MdnsError::Closed));
    }
    assert_eq!(gate.state(), ProbeControllerState::Finished);
}

#[test]
fn conflict_enters_a_five_second_cooldown_after_actual_completion() {
    let mut gate = cooling_at(2);
    assert_eq!(gate.state(), ProbeControllerState::CoolingDown);
    assert_eq!(gate.remaining(seconds(2)), Some(seconds(5)));
    assert_eq!(gate.remaining(seconds(6)), Some(seconds(1)));
    assert_eq!(gate.begin(seconds(6)), Err(MdnsError::CoolingDown));
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.begin(seconds(7)), Ok(()));
    assert_eq!(gate.attempts_started(), 2);
}

#[test]
fn a_nanosecond_short_of_cooldown_cannot_start_the_next_attempt() {
    let mut gate = cooling_at(1);
    let before = seconds(6) - Duration::from_nanos(1);
    assert_eq!(gate.begin(before), Err(MdnsError::CoolingDown));
    assert_eq!(gate.state(), ProbeControllerState::CoolingDown);
    assert_eq!(gate.remaining(before), Some(Duration::from_nanos(1)));
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.begin(seconds(6)), Ok(()));
    assert_eq!(gate.attempts_started(), 2);
}

#[test]
fn repeated_cooldown_checks_neither_spend_attempts_nor_extend_the_due_time() {
    let mut gate = cooling_at(0);
    for now in [0, 0, 1, 2, 3, 4, 4] {
        assert_eq!(gate.begin(seconds(now)), Err(MdnsError::CoolingDown));
        assert_eq!(gate.attempts_started(), 1);
    }
    assert_eq!(gate.remaining(seconds(4)), Some(seconds(1)));
    assert_eq!(gate.begin(seconds(5)), Ok(()));
}

#[test]
fn late_completion_does_not_inherit_the_attempt_start_deadline() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(4)).unwrap();
    gate.finish(seconds(10), true).unwrap();
    assert_eq!(gate.begin(seconds(14)), Err(MdnsError::CoolingDown));
    assert_eq!(gate.remaining(seconds(14)), Some(seconds(1)));
    assert_eq!(gate.begin(seconds(15)), Ok(()));
}

#[test]
fn late_call_starts_at_most_one_attempt_without_catchup() {
    let mut gate = cooling_at(1);
    assert_eq!(gate.begin(seconds(20)), Ok(()));
    assert_eq!(gate.attempts_started(), 2);
    assert_eq!(gate.begin(seconds(20)), Err(MdnsError::Closed));
    assert_eq!(gate.attempts_started(), 2);
}

#[test]
fn each_of_four_conflicting_attempts_is_counted_and_the_last_closes() {
    let mut gate = RetryGate::new();
    for attempt in 0..MAX_ATTEMPTS {
        let now = seconds(u64::from(attempt) * 5);
        assert_eq!(gate.begin(now), Ok(()));
        assert_eq!(gate.attempts_started(), attempt + 1);
        assert_eq!(gate.finish(now, true), Ok(()));
        assert_eq!(
            gate.state(),
            if attempt + 1 == MAX_ATTEMPTS {
                ProbeControllerState::Closed
            } else {
                ProbeControllerState::CoolingDown
            }
        );
    }
    assert_eq!(gate.remaining(seconds(20)), None);
    assert_eq!(gate.begin(seconds(20)), Err(MdnsError::Closed));
    assert_eq!(gate.attempts_started(), 4);
}

#[test]
fn quiet_completion_on_the_fourth_attempt_finishes_instead_of_retrying() {
    let mut gate = RetryGate::new();
    for now in [0, 5, 10] {
        gate.begin(seconds(now)).unwrap();
        gate.finish(seconds(now), true).unwrap();
    }
    gate.begin(seconds(15)).unwrap();
    assert_eq!(gate.finish(seconds(16), false), Ok(()));
    assert_eq!(gate.state(), ProbeControllerState::Finished);
    assert_eq!(gate.attempts_started(), MAX_ATTEMPTS);
}

#[test]
fn cooldown_does_not_extend_the_original_thirty_second_lifetime() {
    let mut gate = cooling_at(29);
    assert_eq!(gate.remaining(seconds(29)), Some(seconds(5)));
    assert_eq!(gate.begin(seconds(30)), Err(MdnsError::DeadlineExceeded));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.remaining(seconds(30)), None);
    assert_eq!(gate.attempts_started(), 1);
}

#[test]
fn an_inflight_completion_at_the_lifetime_boundary_closes() {
    for conflict in [false, true] {
        let mut gate = RetryGate::new();
        gate.begin(seconds(29)).unwrap();
        assert_eq!(
            gate.finish(seconds(30), conflict),
            Err(MdnsError::DeadlineExceeded)
        );
        assert_eq!(gate.state(), ProbeControllerState::Closed);
        assert_eq!(gate.remaining(seconds(30)), None);
    }
}

#[test]
fn time_cannot_move_backwards_while_an_attempt_is_inflight() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(3)).unwrap();
    assert_eq!(gate.finish(seconds(2), false), Err(MdnsError::ClockInvalid));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.begin(seconds(8)), Err(MdnsError::Closed));
}

#[test]
fn rejected_early_begin_still_advances_the_time_watermark() {
    let mut gate = cooling_at(1);
    assert_eq!(gate.begin(seconds(4)), Err(MdnsError::CoolingDown));
    assert_eq!(gate.begin(seconds(3)), Err(MdnsError::ClockInvalid));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.attempts_started(), 1);
}

#[test]
fn second_attempt_finish_cannot_precede_its_actual_begin() {
    let mut gate = cooling_at(0);
    gate.begin(seconds(6)).unwrap();
    assert_eq!(gate.finish(seconds(5), true), Err(MdnsError::ClockInvalid));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.attempts_started(), 2);
}

#[test]
fn duplicate_completion_during_cooldown_closes_instead_of_moving_the_due_time() {
    let mut gate = cooling_at(0);
    assert_eq!(gate.finish(seconds(1), true), Err(MdnsError::Closed));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.remaining(seconds(1)), None);
    assert_eq!(gate.begin(seconds(5)), Err(MdnsError::Closed));
}

#[test]
fn overflowing_cooldown_arithmetic_is_a_terminal_clock_failure() {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    assert_eq!(
        gate.finish(Duration::MAX, true),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.remaining(Duration::MAX), None);
}

#[test]
fn quiet_completion_far_after_deadline_is_not_a_success() {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    assert_eq!(
        gate.finish(Duration::MAX, false),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(gate.state(), ProbeControllerState::Closed);
}

#[test]
fn explicit_close_is_terminal_from_every_nonterminal_phase() {
    let ready = RetryGate::new();
    let mut inflight = RetryGate::new();
    inflight.begin(Duration::ZERO).unwrap();
    let cooling = cooling_at(1);
    for mut gate in [ready, inflight, cooling] {
        let attempts = gate.attempts_started();
        gate.close();
        gate.close();
        assert_eq!(gate.state(), ProbeControllerState::Closed);
        assert_eq!(gate.attempts_started(), attempts);
        assert_eq!(gate.remaining(seconds(100)), None);
        assert_eq!(gate.begin(Duration::ZERO), Err(MdnsError::Closed));
        assert_eq!(gate.finish(Duration::ZERO, false), Err(MdnsError::Closed));
    }
}

#[test]
fn closing_a_finished_gate_does_not_create_another_attempt() {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    gate.finish(seconds(1), false).unwrap();
    gate.close();
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.begin(seconds(6)), Err(MdnsError::Closed));
}

#[test]
fn a_zero_remaining_hint_does_not_admit_an_attempt_or_advance_state() {
    let mut gate = cooling_at(1);
    assert_eq!(gate.remaining(seconds(100)), Some(Duration::ZERO));
    assert_eq!(gate.state(), ProbeControllerState::CoolingDown);
    assert_eq!(gate.attempts_started(), 1);
    assert_eq!(gate.begin(seconds(2)), Err(MdnsError::CoolingDown));
    assert_eq!(gate.begin(seconds(6)), Ok(()));
}

#[test]
fn a_past_hint_does_not_roll_back_the_admission_watermark() {
    let mut gate = cooling_at(2);
    assert_eq!(gate.begin(seconds(6)), Err(MdnsError::CoolingDown));
    assert_eq!(gate.remaining(Duration::ZERO), Some(seconds(7)));
    assert_eq!(gate.begin(seconds(5)), Err(MdnsError::ClockInvalid));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
}

#[test]
fn remaining_hint_saturates_without_closing_an_expired_cooldown() {
    let gate = cooling_at(1);
    assert_eq!(gate.remaining(Duration::MAX), Some(Duration::ZERO));
    assert_eq!(gate.state(), ProbeControllerState::CoolingDown);
    assert_eq!(gate.attempts_started(), 1);
}

#[test]
fn inflight_checks_allow_equal_and_advancing_time_without_new_admissions() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(1)).unwrap();
    for now in [1, 1, 2, 3, 3] {
        assert_eq!(gate.check_in_flight(seconds(now)), Ok(()));
        assert_eq!(gate.attempts_started(), 1);
        assert_eq!(gate.remaining(seconds(now)), None);
    }
    assert_eq!(gate.finish(seconds(3), false), Ok(()));
    assert_eq!(gate.state(), ProbeControllerState::Finished);
}

#[test]
fn inflight_check_rejects_rollback_and_permanently_closes() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(2)).unwrap();
    gate.check_in_flight(seconds(4)).unwrap();
    assert_eq!(
        gate.check_in_flight(seconds(3)),
        Err(MdnsError::ClockInvalid)
    );
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.check_in_flight(seconds(4)), Err(MdnsError::Closed));
    assert_eq!(gate.finish(seconds(4), false), Err(MdnsError::Closed));
}

#[test]
fn inflight_checks_advance_the_watermark_seen_by_finish() {
    let mut gate = RetryGate::new();
    gate.begin(Duration::ZERO).unwrap();
    gate.check_in_flight(seconds(5)).unwrap();
    assert_eq!(gate.finish(seconds(4), true), Err(MdnsError::ClockInvalid));
    assert_eq!(gate.state(), ProbeControllerState::Closed);
}

#[test]
fn inflight_check_observes_the_exact_lifetime_boundary() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(29)).unwrap();
    assert_eq!(
        gate.check_in_flight(MAX_LIFETIME - Duration::from_nanos(1)),
        Ok(())
    );
    assert_eq!(
        gate.check_in_flight(MAX_LIFETIME),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(gate.state(), ProbeControllerState::Closed);
    assert_eq!(gate.check_in_flight(MAX_LIFETIME), Err(MdnsError::Closed));
}

#[test]
fn inflight_check_cannot_be_used_in_ready_or_cooldown_phase() {
    for mut gate in [RetryGate::new(), cooling_at(1)] {
        let attempts = gate.attempts_started();
        assert_eq!(gate.check_in_flight(seconds(6)), Err(MdnsError::Closed));
        assert_eq!(gate.state(), ProbeControllerState::Closed);
        assert_eq!(gate.attempts_started(), attempts);
        assert_eq!(gate.begin(seconds(6)), Err(MdnsError::Closed));
        assert_eq!(gate.remaining(seconds(6)), None);
    }
}

#[test]
fn inflight_check_preserves_a_finished_terminal_state() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(1)).unwrap();
    gate.finish(seconds(2), false).unwrap();
    for now in [Duration::ZERO, seconds(30), Duration::MAX] {
        assert_eq!(gate.check_in_flight(now), Err(MdnsError::Closed));
        assert_eq!(gate.state(), ProbeControllerState::Finished);
        assert_eq!(gate.attempts_started(), 1);
    }
}

#[test]
fn inflight_check_after_explicit_close_never_observes_supplied_time() {
    let mut gate = RetryGate::new();
    gate.begin(seconds(5)).unwrap();
    gate.close();
    for now in [Duration::ZERO, seconds(30), Duration::MAX] {
        assert_eq!(gate.check_in_flight(now), Err(MdnsError::Closed));
        assert_eq!(gate.state(), ProbeControllerState::Closed);
    }
}

#[test]
fn inflight_check_on_a_later_attempt_keeps_the_original_lifetime() {
    let mut gate = cooling_at(20);
    gate.begin(seconds(29)).unwrap();
    assert_eq!(gate.check_in_flight(seconds(29)), Ok(()));
    assert_eq!(
        gate.check_in_flight(seconds(30)),
        Err(MdnsError::DeadlineExceeded)
    );
    assert_eq!(gate.attempts_started(), 2);
    assert_eq!(gate.state(), ProbeControllerState::Closed);
}
