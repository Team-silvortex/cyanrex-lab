use crate::models::event::{EventCategory, EventColor, EventSeverity};

pub(crate) fn parse_category(raw: &str) -> EventCategory {
    match raw {
        "kernel" => EventCategory::Kernel,
        _ => EventCategory::Platform,
    }
}

pub(crate) fn parse_severity(raw: &str) -> EventSeverity {
    match raw {
        "success" => EventSeverity::Success,
        "warning" => EventSeverity::Warning,
        _ => EventSeverity::Error,
    }
}

pub(crate) fn parse_color(raw: &str) -> EventColor {
    match raw {
        "green" => EventColor::Green,
        "yellow" => EventColor::Yellow,
        _ => EventColor::Red,
    }
}
