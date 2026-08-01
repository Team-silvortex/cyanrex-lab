use super::event_bus::EventOverflowPolicy;

pub(crate) fn parse_policy(value: &str) -> EventOverflowPolicy {
    if value.eq_ignore_ascii_case("drop_new") {
        EventOverflowPolicy::DropNew
    } else {
        EventOverflowPolicy::DropOldest
    }
}

pub(crate) fn policy_to_str(policy: EventOverflowPolicy) -> &'static str {
    match policy {
        EventOverflowPolicy::DropOldest => "drop_oldest",
        EventOverflowPolicy::DropNew => "drop_new",
    }
}
