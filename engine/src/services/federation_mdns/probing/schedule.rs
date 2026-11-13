//! Caller-driven one-attempt timing only; no clock, I/O, sleeping or publication authority.
use crate::services::federation_mdns::MdnsError;

#[derive(Debug)]
pub(super) struct ProbeSchedule {
    sent_count: u8,
    due_ms: u64,
    watermark_ms: u64,
    pending: bool,
    closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Wait(u64),
    Send,
    Complete,
}

impl ProbeSchedule {
    pub(super) fn new(jitter_ms: u16) -> Result<Self, MdnsError> {
        if jitter_ms > 250 {
            return Err(MdnsError::InvalidConfig);
        }
        Ok(Self {
            sent_count: 0,
            due_ms: u64::from(jitter_ms),
            watermark_ms: 0,
            pending: false,
            closed: false,
        })
    }

    /// Return at most one action. Sending must be acknowledged before asking for another action.
    pub(super) fn step(&mut self, elapsed_ms: u64) -> Result<Step, MdnsError> {
        self.observe_time(elapsed_ms)?;
        if self.pending {
            return self.close(MdnsError::Closed);
        }
        if elapsed_ms < self.due_ms {
            return Ok(Step::Wait(self.due_ms - elapsed_ms));
        }
        if self.sent_count == 3 {
            self.closed = true;
            return Ok(Step::Complete);
        }
        self.pending = true;
        Ok(Step::Send)
    }

    /// Acknowledge actual completion, not an intended deadline: late sends never catch up.
    pub(super) fn sent(&mut self, elapsed_ms: u64) -> Result<(), MdnsError> {
        self.observe_time(elapsed_ms)?;
        if !self.pending {
            return self.close(MdnsError::Closed);
        }
        let Some(due_ms) = elapsed_ms.checked_add(250) else {
            return self.close(MdnsError::ClockInvalid);
        };
        self.sent_count += 1;
        self.pending = false;
        self.due_ms = due_ms;
        Ok(())
    }

    pub(super) fn sent_count(&self) -> u8 {
        self.sent_count
    }

    fn observe_time(&mut self, elapsed_ms: u64) -> Result<(), MdnsError> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        if elapsed_ms < self.watermark_ms {
            return self.close(MdnsError::ClockInvalid);
        }
        self.watermark_ms = elapsed_ms;
        Ok(())
    }

    fn close<T>(&mut self, error: MdnsError) -> Result<T, MdnsError> {
        self.pending = false;
        self.closed = true;
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(jitter: u16) -> ProbeSchedule {
        ProbeSchedule::new(jitter).unwrap()
    }

    fn send_at(schedule: &mut ProbeSchedule, elapsed: u64) {
        assert_eq!(schedule.step(elapsed), Ok(Step::Send));
        schedule.sent(elapsed).unwrap();
    }

    fn assert_closed(schedule: &mut ProbeSchedule, elapsed: u64) {
        assert_eq!(schedule.step(elapsed), Err(MdnsError::Closed));
        assert_eq!(schedule.sent(elapsed), Err(MdnsError::Closed));
    }

    #[test]
    fn every_jitter_from_zero_through_250_is_accepted() {
        for jitter in 0..=250 {
            let mut schedule = opened(jitter);
            assert_eq!(schedule.sent_count(), 0);
            assert_eq!(schedule.step(u64::from(jitter)), Ok(Step::Send));
        }
    }

    #[test]
    fn jitter_above_250_is_invalid_before_a_schedule_exists() {
        for jitter in [251, 252, 1_000, u16::MAX] {
            assert!(matches!(
                ProbeSchedule::new(jitter),
                Err(MdnsError::InvalidConfig)
            ));
        }
    }

    #[test]
    fn zero_jitter_is_ready_at_zero_without_a_sleep_action() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(0), Ok(Step::Send));
        assert_eq!(schedule.sent_count(), 0);
    }

    #[test]
    fn maximum_jitter_returns_only_the_remaining_initial_wait() {
        let mut schedule = opened(250);
        assert_eq!(schedule.step(0), Ok(Step::Wait(250)));
        assert_eq!(schedule.step(100), Ok(Step::Wait(150)));
        assert_eq!(schedule.step(249), Ok(Step::Wait(1)));
        assert_eq!(schedule.step(250), Ok(Step::Send));
    }

    #[test]
    fn equal_time_wait_steps_are_legal_and_do_not_consume_sends() {
        let mut schedule = opened(1);
        for _ in 0..5 {
            assert_eq!(schedule.step(0), Ok(Step::Wait(1)));
            assert_eq!(schedule.sent_count(), 0);
        }
        send_at(&mut schedule, 1);
        assert_eq!(schedule.sent_count(), 1);
    }

    #[test]
    fn next_probe_is_not_due_249_ms_after_an_acknowledged_send() {
        let mut schedule = opened(0);
        send_at(&mut schedule, 0);
        assert_eq!(schedule.step(249), Ok(Step::Wait(1)));
        assert_eq!(schedule.step(250), Ok(Step::Send));
    }

    #[test]
    fn next_due_is_250_ms_after_actual_send_ack_not_original_due() {
        let mut schedule = opened(100);
        assert_eq!(schedule.step(100), Ok(Step::Send));
        schedule.sent(175).unwrap();
        assert_eq!(schedule.step(350), Ok(Step::Wait(75)));
        assert_eq!(schedule.step(424), Ok(Step::Wait(1)));
        assert_eq!(schedule.step(425), Ok(Step::Send));
    }

    #[test]
    fn exactly_three_successful_sends_precede_completion() {
        let mut schedule = opened(0);
        for (count, elapsed) in [0, 250, 500].into_iter().enumerate() {
            send_at(&mut schedule, elapsed);
            assert_eq!(schedule.sent_count(), count as u8 + 1);
        }
        assert_eq!(schedule.step(750), Ok(Step::Complete));
        assert_eq!(schedule.sent_count(), 3);
    }

    #[test]
    fn third_send_still_requires_the_entire_final_250_ms_wait() {
        let mut schedule = opened(250);
        for elapsed in [250, 500, 750] {
            send_at(&mut schedule, elapsed);
        }
        assert_eq!(schedule.step(750), Ok(Step::Wait(250)));
        assert_eq!(schedule.step(999), Ok(Step::Wait(1)));
        assert_eq!(schedule.step(1_000), Ok(Step::Complete));
    }

    #[test]
    fn late_first_send_does_not_catch_up_the_next_two() {
        let mut schedule = opened(0);
        send_at(&mut schedule, 1_000);
        assert_eq!(schedule.step(1_000), Ok(Step::Wait(250)));
        send_at(&mut schedule, 1_250);
        assert_eq!(schedule.step(1_250), Ok(Step::Wait(250)));
        send_at(&mut schedule, 1_500);
        assert_eq!(schedule.step(1_500), Ok(Step::Wait(250)));
        assert_eq!(schedule.step(1_750), Ok(Step::Complete));
    }

    #[test]
    fn late_each_step_never_sends_more_than_one_action() {
        let mut schedule = opened(0);
        for elapsed in [1_000, 10_000, 100_000] {
            send_at(&mut schedule, elapsed);
            assert_eq!(schedule.step(elapsed), Ok(Step::Wait(250)));
        }
        assert_eq!(schedule.step(1_000_000), Ok(Step::Complete));
    }

    #[test]
    fn late_third_ack_controls_the_final_deadline() {
        let mut schedule = opened(0);
        send_at(&mut schedule, 0);
        send_at(&mut schedule, 250);
        assert_eq!(schedule.step(500), Ok(Step::Send));
        schedule.sent(900).unwrap();
        assert_eq!(schedule.step(1_149), Ok(Step::Wait(1)));
        assert_eq!(schedule.step(1_150), Ok(Step::Complete));
    }

    #[test]
    fn repeated_step_while_send_is_pending_closes_permanently() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(0), Ok(Step::Send));
        assert_eq!(schedule.step(0), Err(MdnsError::Closed));
        assert_eq!(schedule.sent_count(), 0);
        assert_closed(&mut schedule, 250);
    }

    #[test]
    fn delayed_repeated_step_while_send_is_pending_also_closes() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(0), Ok(Step::Send));
        assert_eq!(schedule.step(250), Err(MdnsError::Closed));
        assert_closed(&mut schedule, 500);
    }

    #[test]
    fn ack_without_a_pending_send_closes_without_increment() {
        let mut schedule = opened(100);
        assert_eq!(schedule.sent(0), Err(MdnsError::Closed));
        assert_eq!(schedule.sent_count(), 0);
        assert_closed(&mut schedule, 100);
    }

    #[test]
    fn duplicate_ack_closes_and_preserves_only_the_original_send() {
        let mut schedule = opened(0);
        send_at(&mut schedule, 0);
        assert_eq!(schedule.sent(0), Err(MdnsError::Closed));
        assert_eq!(schedule.sent_count(), 1);
        assert_closed(&mut schedule, 250);
    }

    #[test]
    fn completion_is_returned_once_and_is_not_reusable() {
        let mut schedule = opened(0);
        for elapsed in [0, 250, 500] {
            send_at(&mut schedule, elapsed);
        }
        assert_eq!(schedule.step(750), Ok(Step::Complete));
        assert_closed(&mut schedule, 750);
        assert_closed(&mut schedule, 1_000);
        assert_eq!(schedule.sent_count(), 3);
    }

    #[test]
    fn completion_remains_closed_even_if_a_later_time_sample_rolls_back() {
        let mut schedule = opened(0);
        for elapsed in [0, 250, 500] {
            send_at(&mut schedule, elapsed);
        }
        assert_eq!(schedule.step(750), Ok(Step::Complete));
        assert_closed(&mut schedule, 0);
    }

    #[test]
    fn rollback_between_wait_steps_is_clock_invalid_and_terminal() {
        let mut schedule = opened(250);
        assert_eq!(schedule.step(100), Ok(Step::Wait(150)));
        assert_eq!(schedule.step(99), Err(MdnsError::ClockInvalid));
        assert_closed(&mut schedule, 250);
    }

    #[test]
    fn rollback_in_send_ack_is_clock_invalid_and_does_not_count_a_send() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(100), Ok(Step::Send));
        assert_eq!(schedule.sent(99), Err(MdnsError::ClockInvalid));
        assert_eq!(schedule.sent_count(), 0);
        assert_closed(&mut schedule, 100);
    }

    #[test]
    fn acknowledged_time_is_part_of_the_next_step_watermark() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(100), Ok(Step::Send));
        schedule.sent(150).unwrap();
        assert_eq!(schedule.step(149), Err(MdnsError::ClockInvalid));
        assert_eq!(schedule.sent_count(), 1);
        assert_closed(&mut schedule, 400);
    }

    #[test]
    fn rollback_while_pending_is_clock_invalid_before_the_protocol_check() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(100), Ok(Step::Send));
        assert_eq!(schedule.step(99), Err(MdnsError::ClockInvalid));
        assert_closed(&mut schedule, 100);
    }

    #[test]
    fn rollback_in_an_unexpected_ack_is_clock_invalid_before_protocol_check() {
        let mut schedule = opened(250);
        assert_eq!(schedule.step(100), Ok(Step::Wait(150)));
        assert_eq!(schedule.sent(99), Err(MdnsError::ClockInvalid));
        assert_closed(&mut schedule, 250);
    }

    #[test]
    fn overflowing_next_deadline_is_clock_invalid_and_not_a_successful_ack() {
        let mut schedule = opened(0);
        assert_eq!(schedule.step(u64::MAX - 249), Ok(Step::Send));
        assert_eq!(schedule.sent(u64::MAX - 249), Err(MdnsError::ClockInvalid));
        assert_eq!(schedule.sent_count(), 0);
        assert_closed(&mut schedule, u64::MAX);
    }

    #[test]
    fn exact_maximum_deadline_is_valid_without_wrapping() {
        let mut schedule = opened(0);
        send_at(&mut schedule, u64::MAX - 250);
        assert_eq!(schedule.step(u64::MAX - 1), Ok(Step::Wait(1)));
        assert_eq!(schedule.step(u64::MAX), Ok(Step::Send));
        assert_eq!(schedule.sent(u64::MAX), Err(MdnsError::ClockInvalid));
        assert_eq!(schedule.sent_count(), 1);
        assert_closed(&mut schedule, u64::MAX);
    }

    #[test]
    fn final_wait_deadline_overflow_also_closes_without_third_ack() {
        let mut schedule = opened(0);
        send_at(&mut schedule, 0);
        send_at(&mut schedule, 250);
        assert_eq!(schedule.step(u64::MAX), Ok(Step::Send));
        assert_eq!(schedule.sent(u64::MAX), Err(MdnsError::ClockInvalid));
        assert_eq!(schedule.sent_count(), 2);
        assert_closed(&mut schedule, u64::MAX);
    }
}
