use crate::models::event::{EventCategory, EventColor, EventSeverity};

pub(crate) fn to_category_str(category: EventCategory) -> &'static str {
    match category {
        EventCategory::Kernel => "kernel",
        EventCategory::Platform => "platform",
    }
}

pub(crate) fn to_severity_str(severity: EventSeverity) -> &'static str {
    match severity {
        EventSeverity::Success => "success",
        EventSeverity::Warning => "warning",
        EventSeverity::Error => "error",
    }
}

pub(crate) fn to_color_str(color: EventColor) -> &'static str {
    match color {
        EventColor::Green => "green",
        EventColor::Yellow => "yellow",
        EventColor::Red => "red",
    }
}
