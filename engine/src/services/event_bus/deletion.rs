use super::*;
use crate::services::event_bus_filter::{
    matches_event_filters, normalize_category_filter, normalize_severity_filter,
};

impl EventBus {
    pub(super) async fn delete_from_history_filtered(
        &self,
        username: &str,
        category: Option<&str>,
        severity: Option<&str>,
        since_minutes: Option<i64>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> usize {
        let category_filter = normalize_category_filter(category);
        let severity_filter = normalize_severity_filter(severity);
        if (category.is_some() && category_filter.is_none())
            || (severity.is_some() && severity_filter.is_none())
        {
            return 0;
        }
        let since_cutoff = match since_minutes {
            Some(minutes) if minutes > 0 => {
                let Some(cutoff) = chrono::Duration::try_minutes(minutes)
                    .and_then(|duration| Utc::now().checked_sub_signed(duration))
                else {
                    return 0;
                };
                Some(cutoff)
            }
            Some(minutes) if minutes < 0 => return 0,
            _ => None,
        };
        // Never replace a snapshot or write it back to SQL: that can erase concurrent publications
        // and undo an already completed SQL DELETE. Take both locks before mutating, so cancellation
        // cannot leave history and the unread suffix half-updated.
        let mut history = self.history.write().await;
        let mut unread = self.unread.write().await;
        let Some(events) = history.get_mut(username) else {
            return 0;
        };
        let before = events.len();
        let unread_count = unread.entry(username.to_string()).or_insert(0);
        let unread_start = before.saturating_sub(*unread_count);
        let mut index = 0;
        let mut unread_deleted = 0;
        events.retain(|event| {
            let remove = matches_event_filters(
                event,
                category_filter.as_deref(),
                severity_filter.as_deref(),
                since_cutoff,
                start.as_ref(),
                end.as_ref(),
            );
            if remove && index >= unread_start {
                unread_deleted += 1;
            }
            index += 1;
            !remove
        });
        *unread_count = unread_count
            .saturating_sub(unread_deleted)
            .min(events.len());
        before - events.len()
    }
}
