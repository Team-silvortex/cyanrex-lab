//! Borrowed projections: full payloads are copied only after selecting a response page.
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use crate::models::learning::{
    LabAttempt, LabDefinition, LabProgress, LabProgressStatus, StudentLearningOverview,
    TeacherLearningOverview,
};
use crate::services::learning_catalog::lab_definitions;
use chrono::{DateTime, Utc};

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;

struct RankedAttempt<'a> {
    attempt: &'a LabAttempt,
    order: usize,
}

impl Ord for RankedAttempt<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        // The max-heap root is the worst retained row: oldest time, then latest input position.
        other
            .attempt
            .created_at
            .cmp(&self.attempt.created_at)
            .then_with(|| self.order.cmp(&other.order))
    }
}
impl PartialOrd for RankedAttempt<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for RankedAttempt<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for RankedAttempt<'_> {}

pub(super) fn select_recent<'a>(
    attempts: impl IntoIterator<Item = &'a LabAttempt>,
    limit: Option<usize>,
) -> Vec<&'a LabAttempt> {
    let Some(limit) = limit else {
        let mut selected: Vec<_> = attempts.into_iter().collect();
        selected.sort_by_key(|row| std::cmp::Reverse(row.created_at));
        return selected;
    };
    if limit == 0 {
        return Vec::new();
    }
    let mut newest = BinaryHeap::with_capacity(limit);
    for (order, attempt) in attempts.into_iter().enumerate() {
        let candidate = RankedAttempt { attempt, order };
        if newest.len() < limit {
            newest.push(candidate);
        } else if newest.peek().is_some_and(|worst| candidate < *worst) {
            newest.pop(); // Evict before insertion so the bounded heap cannot grow.
            newest.push(candidate);
        }
    }
    let mut selected = Vec::with_capacity(newest.len());
    selected.extend(newest.into_sorted_vec().into_iter().map(|row| row.attempt));
    selected
}

#[derive(Clone, Default)]
struct LabStats<'a> {
    count: usize,
    latest: Option<&'a LabAttempt>,
    completed_at: Option<DateTime<Utc>>,
}

impl<'a> LabStats<'a> {
    fn push(&mut self, attempt: &'a LabAttempt) {
        self.count += 1;
        // Strict comparison preserves the first input record for equal timestamps.
        if self
            .latest
            .map_or(true, |latest| attempt.created_at > latest.created_at)
        {
            self.latest = Some(attempt);
        }
        if attempt.completed {
            self.completed_at = Some(
                self.completed_at
                    .map_or(attempt.created_at, |time| time.min(attempt.created_at)),
            );
        }
    }

    fn finish(self, lab: LabDefinition) -> LabProgress {
        LabProgress {
            lab,
            status: if self.completed_at.is_some() {
                LabProgressStatus::Completed
            } else if self.count == 0 {
                LabProgressStatus::NotStarted
            } else {
                LabProgressStatus::InProgress
            },
            attempts: self.count as u32,
            latest_stage: self.latest.map(|row| row.stage.clone()),
            latest_feedback: self
                .latest
                .map(|row| row.feedback.clone())
                .unwrap_or_default(),
            last_attempt_at: self.latest.map(|row| row.created_at),
            completed_at: self.completed_at,
        }
    }
}

pub(super) fn progress_from_attempts<'a>(
    attempts: impl IntoIterator<Item = &'a LabAttempt>,
) -> Vec<LabProgress> {
    let labs = lab_definitions();
    let mut stats = vec![LabStats::default(); labs.len()];
    for attempt in attempts {
        if let Some(index) = labs.iter().position(|lab| lab.id == attempt.lab_id) {
            stats[index].push(attempt);
        }
    }
    stats
        .into_iter()
        .zip(labs)
        .map(|(stats, lab)| stats.finish(lab))
        .collect()
}

struct StudentStats<'a> {
    count: usize,
    last_activity_at: Option<DateTime<Utc>>,
    labs: Vec<LabStats<'a>>,
}

pub(super) fn teacher_overview_from_attempts<'a>(
    attempts: impl IntoIterator<Item = &'a LabAttempt>,
) -> TeacherLearningOverview {
    let labs = lab_definitions();
    let mut grouped: HashMap<&str, StudentStats<'_>> = HashMap::new();
    for attempt in attempts {
        let student = grouped
            .entry(&attempt.username)
            .or_insert_with(|| StudentStats {
                count: 0,
                last_activity_at: None,
                labs: vec![LabStats::default(); labs.len()],
            });
        student.count += 1;
        student.last_activity_at = Some(
            student
                .last_activity_at
                .map_or(attempt.created_at, |time| time.max(attempt.created_at)),
        );
        // Unknown/retired labs still count toward student totals and activity, as before.
        if let Some(index) = labs.iter().position(|lab| lab.id == attempt.lab_id) {
            student.labs[index].push(attempt);
        }
    }
    let total_labs = labs.len() as u32;
    let mut students: Vec<_> = grouped
        .into_iter()
        .map(|(username, stats)| {
            let labs: Vec<_> = stats
                .labs
                .into_iter()
                .zip(labs.iter().cloned())
                .map(|(stats, lab)| stats.finish(lab))
                .collect();
            StudentLearningOverview {
                username: username.into(),
                total_labs,
                completed_labs: labs
                    .iter()
                    .filter(|lab| lab.status == LabProgressStatus::Completed)
                    .count() as u32,
                total_attempts: stats.count as u32,
                last_activity_at: stats.last_activity_at,
                labs,
            }
        })
        .collect();
    students.sort_by(|a, b| {
        b.last_activity_at
            .cmp(&a.last_activity_at)
            .then_with(|| a.username.cmp(&b.username))
    });
    TeacherLearningOverview {
        generated_at: Utc::now(),
        total_labs,
        active_students: students.len() as u32,
        students,
    }
}
