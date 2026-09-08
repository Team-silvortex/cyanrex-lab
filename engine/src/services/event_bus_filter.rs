use chrono::{DateTime, Utc};

use crate::models::event::{Event, EventCategory, EventSeverity};
use crate::services::event_bus::EventQueryFilters;

// Newest-first by insertion order. Select references before callers clone the bounded result.
pub(crate) fn latest_matching_events<'a>(
    events: impl DoubleEndedIterator<Item = &'a Event>,
    max_records: usize,
    filters: EventQueryFilters<'_>,
) -> impl Iterator<Item = &'a Event> {
    let category = normalize_category_filter(filters.category);
    let severity = normalize_severity_filter(filters.severity);
    let since_cutoff = filters
        .since_minutes
        .filter(|minutes| *minutes > 0)
        .map(|minutes| Utc::now() - chrono::Duration::minutes(minutes));
    let (start, end) = (filters.start, filters.end);
    events
        .rev()
        .take(max_records)
        .filter(move |event| {
            matches_event_filters(
                event,
                category.as_deref(),
                severity.as_deref(),
                since_cutoff,
                start.as_ref(),
                end.as_ref(),
            )
        })
        .take(filters.limit.unwrap_or(usize::MAX))
}

pub(crate) fn normalize_category_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "kernel" | "platform"))
}

pub(crate) fn normalize_severity_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "success" | "warning" | "error"))
}

pub(crate) fn matches_event_filters(
    event: &Event,
    category: Option<&str>,
    severity: Option<&str>,
    since_cutoff: Option<DateTime<Utc>>,
    start: Option<&DateTime<Utc>>,
    end: Option<&DateTime<Utc>>,
) -> bool {
    if let Some(category_filter) = category {
        let expected = match category_filter {
            "kernel" => EventCategory::Kernel,
            "platform" => EventCategory::Platform,
            _ => return false,
        };
        if event.category != expected {
            return false;
        }
    }

    if let Some(severity_filter) = severity {
        let expected = match severity_filter {
            "success" => EventSeverity::Success,
            "warning" => EventSeverity::Warning,
            "error" => EventSeverity::Error,
            _ => return false,
        };
        if event.severity != expected {
            return false;
        }
    }

    if let Some(cutoff) = since_cutoff {
        if event.timestamp < cutoff {
            return false;
        }
    }

    if let Some(start_time) = start {
        if event.timestamp < *start_time {
            return false;
        }
    }

    if let Some(end_time) = end {
        if event.timestamp > *end_time {
            return false;
        }
    }

    true
}

pub(crate) fn filter_events(
    mut events: Vec<Event>,
    category: Option<&str>,
    severity: Option<&str>,
    limit: Option<usize>,
    since_minutes: Option<i64>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Vec<Event> {
    let category_filter = normalize_category_filter(category);
    let severity_filter = normalize_severity_filter(severity);
    let since_cutoff = since_minutes.filter(|value| *value > 0).map(|value| {
        // UTC now is intentionally evaluated at filter time so in-memory fallback
        // follows the same semantics as DB-side filter.
        Utc::now() - chrono::Duration::minutes(value)
    });

    events.retain(|event| {
        matches_event_filters(
            event,
            category_filter.as_deref(),
            severity_filter.as_deref(),
            since_cutoff,
            start.as_ref(),
            end.as_ref(),
        )
    });

    if let Some(max) = limit {
        if events.len() > max {
            events = events.split_off(events.len() - max);
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{cell::Cell, collections::VecDeque};

    fn event(seq: usize, now: DateTime<Utc>) -> Event {
        let severity = if seq % 3 == 0 {
            EventSeverity::Warning
        } else {
            EventSeverity::Success
        };
        Event {
            username: "alice".into(),
            timestamp: now - chrono::Duration::seconds((seq % 11 * 60 + 30) as i64),
            source: "query-test".into(),
            event_type: "synthetic".into(),
            category: if seq % 10 == 0 {
                EventCategory::Kernel
            } else {
                EventCategory::Platform
            },
            severity,
            color: severity.color(),
            payload: json!({"seq": seq}),
        }
    }

    fn snapshot(
        events: &VecDeque<Event>,
        max_records: usize,
        filters: EventQueryFilters<'_>,
    ) -> Vec<Event> {
        let mut result: Vec<_> = latest_matching_events(events.iter(), max_records, filters)
            .cloned()
            .collect();
        result.reverse();
        result
    }

    #[test]
    fn latest_page_stops_visiting_after_its_limit_and_returns_borrowed_events() {
        let events: VecDeque<_> = (0..5000).map(|seq| event(seq, Utc::now())).collect();
        let visited = Cell::new(0);
        let selected: Vec<_> = latest_matching_events(
            events.iter().inspect(|_| visited.set(visited.get() + 1)),
            events.len(),
            EventQueryFilters {
                limit: Some(200),
                ..Default::default()
            },
        )
        .collect();
        assert_eq!(selected.len(), 200);
        assert_eq!(visited.get(), 200);
        assert!(std::ptr::eq(selected[0], &events[4999]));
        assert!(std::ptr::eq(selected[199], &events[4800]));
    }

    #[test]
    fn zero_limit_does_not_visit_history() {
        let events: VecDeque<_> = (0..100).map(|seq| event(seq, Utc::now())).collect();
        let visited = Cell::new(0);
        let selected: Vec<_> = latest_matching_events(
            events.iter().inspect(|_| visited.set(visited.get() + 1)),
            events.len(),
            EventQueryFilters {
                limit: Some(0),
                ..Default::default()
            },
        )
        .collect();
        assert!(selected.is_empty());
        assert_eq!(visited.get(), 0);
    }

    #[test]
    fn selective_page_stops_after_enough_matches_not_enough_scanned_events() {
        let events: VecDeque<_> = (0..1000).map(|seq| event(seq, Utc::now())).collect();
        let visited = Cell::new(0);
        let selected: Vec<_> = latest_matching_events(
            events.iter().inspect(|_| visited.set(visited.get() + 1)),
            events.len(),
            EventQueryFilters {
                category: Some("kernel"),
                limit: Some(3),
                ..Default::default()
            },
        )
        .collect();
        assert_eq!(
            selected
                .iter()
                .map(|event| event.payload["seq"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![990, 980, 970]
        );
        assert_eq!(visited.get(), 30);
    }

    #[test]
    fn retention_is_applied_before_filtering_older_matches() {
        let mut events: VecDeque<_> = (0..100).map(|seq| event(seq, Utc::now())).collect();
        for event in events.iter_mut().skip(50) {
            event.category = EventCategory::Platform;
        }
        let filters = EventQueryFilters {
            category: Some("kernel"),
            limit: Some(3),
            ..Default::default()
        };
        assert!(snapshot(&events, 50, filters).is_empty());
    }

    #[test]
    fn time_bounds_are_inclusive_and_do_not_assume_timestamp_order() {
        let now = Utc::now();
        let mut events: VecDeque<_> = (0..4).map(|seq| event(seq, now)).collect();
        for (event, seconds) in events.iter_mut().zip([200, 10, 300, 20]) {
            event.timestamp = DateTime::from_timestamp(seconds, 0).unwrap();
        }
        let filters = EventQueryFilters {
            start: DateTime::from_timestamp(150, 0),
            limit: Some(1),
            ..Default::default()
        };
        assert_eq!(snapshot(&events, 4, filters)[0].payload["seq"], 2);
        let exact = DateTime::from_timestamp(200, 0);
        let filters = EventQueryFilters {
            start: exact,
            end: exact,
            ..Default::default()
        };
        let result = snapshot(&events, 4, filters);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].payload["seq"], 0);
    }

    #[test]
    fn borrowed_selection_matches_legacy_filters_across_wrapped_histories() {
        let now = Utc::now();
        let mut events = VecDeque::with_capacity(100);
        events.extend((0..100).map(|seq| event(seq, now)));
        for seq in 100..137 {
            events.pop_front();
            events.push_back(event(seq, now));
        }
        assert!(!events.as_slices().1.is_empty());
        for max_records in [0, 17, 100] {
            for category in [None, Some("kernel"), Some(" PLATFORM "), Some("unknown")] {
                for severity in [None, Some(" WARNING "), Some("error")] {
                    for limit in [None, Some(0), Some(1), Some(7), Some(200)] {
                        for since_minutes in [None, Some(-1), Some(5)] {
                            for (start, end) in [
                                (None, None),
                                (Some(now - chrono::Duration::minutes(7)), None),
                                (None, Some(now - chrono::Duration::minutes(3))),
                                (Some(now), Some(now - chrono::Duration::minutes(1))),
                            ] {
                                let filters = EventQueryFilters {
                                    category,
                                    severity,
                                    limit,
                                    since_minutes,
                                    start,
                                    end,
                                };
                                let source: Vec<_> = events
                                    .iter()
                                    .skip(events.len().saturating_sub(max_records))
                                    .cloned()
                                    .collect();
                                let expected = filter_events(
                                    source,
                                    category,
                                    severity,
                                    limit,
                                    since_minutes,
                                    start,
                                    end,
                                );
                                assert_eq!(
                                    serde_json::to_value(snapshot(&events, max_records, filters))
                                        .unwrap(),
                                    serde_json::to_value(expected).unwrap(),
                                    "{filters:?}, max_records={max_records}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
