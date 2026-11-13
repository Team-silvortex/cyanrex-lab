//! Pure caller-driven planning. No networking, probe result, ownership or dispatch receipt.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::PeerAdvertisement,
    federation_mdns::{AnnouncementStep, MdnsAnnouncement, MdnsError},
};
use std::{net::SocketAddrV4, time::Duration};

#[path = "federation_mdns_announcement_schedule/clocks.rs"]
mod clocks;

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

fn millis(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn announcement(expiry: u64) -> MdnsAnnouncement {
    let raw = serde_json::json!({
        "schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":["peer.identity"],
        "required_capabilities":[],"issued_at":1000,"expires_at":expiry,
        "hints":[{"kind":"direct","address":"192.168.1.7:4433"}]
    });
    let ad = PeerAdvertisement::parse(&serde_json::to_vec(&raw).unwrap()).unwrap();
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        "192.168.1.7:4433".parse::<SocketAddrV4>().unwrap(),
        &ad,
        1000,
        120,
    )
    .unwrap()
}

#[test]
fn first_intent_is_immediate_without_claiming_a_probe_or_send() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Ok(AnnouncementStep::Prepare { ordinal: 1 })
    );
}

#[test]
fn exactly_two_completed_intents_then_one_complete_and_permanent_closure() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Ok(AnnouncementStep::Prepare { ordinal: 1 })
    );
    plan.record_completion(millis(20), seconds(1000), false)
        .unwrap();
    assert_eq!(
        plan.step(millis(1019), seconds(1001), false),
        Ok(AnnouncementStep::Wait(millis(1)))
    );
    assert_eq!(
        plan.step(millis(1020), seconds(1001), false),
        Ok(AnnouncementStep::Prepare { ordinal: 2 })
    );
    plan.record_completion(millis(1030), seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(millis(1030), seconds(1001), false),
        Ok(AnnouncementStep::Complete)
    );
    assert_eq!(
        plan.step(millis(1030), seconds(1001), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.record_completion(millis(1030), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn delayed_first_completion_does_not_catch_up_from_the_first_intent() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(millis(1750), seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(2), seconds(1002), false),
        Ok(AnnouncementStep::Wait(millis(750)))
    );
    assert_eq!(
        plan.step(millis(2750), seconds(1002), false),
        Ok(AnnouncementStep::Prepare { ordinal: 2 })
    );
}

#[test]
fn late_second_step_produces_one_intent_not_a_periodic_burst() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(5)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(4), seconds(1004), false),
        Ok(AnnouncementStep::Prepare { ordinal: 2 })
    );
    plan.record_completion(seconds(4), seconds(1004), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(4), seconds(1004), false),
        Ok(AnnouncementStep::Complete)
    );
    assert_eq!(
        plan.step(seconds(4), seconds(1004), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn a_duplicate_step_while_pending_closes_instead_of_reemitting() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn completion_before_an_intent_permanently_closes() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn duplicate_completion_cannot_skip_wait_or_advance_to_a_third_intent() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn second_pending_intent_also_rejects_reentry() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(seconds(1), seconds(1001), false).unwrap();
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.record_completion(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn explicit_close_is_idempotent_and_does_not_report_completion() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.close();
    plan.close();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn cancellation_before_first_intent_is_terminal() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), true),
        Err(MdnsError::Cancelled)
    );
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn cancellation_while_pending_cannot_be_cleared_by_completion() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), true),
        Err(MdnsError::Cancelled)
    );
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn cancellation_during_wait_never_returns_the_second_intent() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    assert_eq!(
        plan.step(millis(500), seconds(1000), true),
        Err(MdnsError::Cancelled)
    );
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn cancellation_after_second_completion_cannot_become_complete() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(seconds(1), seconds(1001), false).unwrap();
    plan.record_completion(seconds(1), seconds(1001), false)
        .unwrap();
    assert_eq!(
        plan.step(seconds(1), seconds(1001), true),
        Err(MdnsError::Cancelled)
    );
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn construction_and_model_progress_do_not_mutate_the_frozen_wire_declaration() {
    let ad = announcement(1100);
    let before = ad.encode_at(1000).unwrap();
    let goodbye = ad.encode_goodbye().unwrap();
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.close();
    assert_eq!(ad.encode_at(1000).unwrap(), before);
    assert_eq!(ad.encode_goodbye().unwrap(), goodbye);
}

#[test]
fn cancellation_during_the_second_completion_is_terminal() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.step(seconds(1), seconds(1001), false).unwrap();
    assert_eq!(
        plan.record_completion(seconds(1), seconds(1001), true),
        Err(MdnsError::Cancelled)
    );
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn explicit_close_during_wait_cannot_be_reopened_or_completed() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.step(Duration::ZERO, seconds(1000), false).unwrap();
    plan.record_completion(Duration::ZERO, seconds(1000), false)
        .unwrap();
    plan.close();
    assert_eq!(
        plan.step(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.record_completion(seconds(1), seconds(1001), false),
        Err(MdnsError::Closed)
    );
}

#[test]
fn closed_state_takes_precedence_over_a_new_cancellation_sample() {
    let ad = announcement(1100);
    let mut plan = ad.initial_schedule(seconds(1000), seconds(2)).unwrap();
    plan.close();
    assert_eq!(
        plan.step(Duration::ZERO, seconds(1000), true),
        Err(MdnsError::Closed)
    );
    assert_eq!(
        plan.record_completion(Duration::ZERO, seconds(1000), true),
        Err(MdnsError::Closed)
    );
}
