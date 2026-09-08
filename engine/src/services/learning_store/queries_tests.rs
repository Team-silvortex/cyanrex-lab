use super::*;
use crate::models::learning::{LabProgressStatus, LabTeacherFeedback};
use serde_json::{json, Value};

fn attempt(index: usize) -> LabAttempt {
    LabAttempt {
        id: format!("row-{index}"),
        username: format!("student-{}", index % 7),
        lab_id: lab_definitions()[index % 5].id.clone(),
        template_id: None,
        source: "not used by aggregates".repeat(100),
        source_sha256: "hash".into(),
        run_success: index % 4 == 0,
        stage: format!("stage-{index}"),
        attach_expected: false,
        attach_verified: false,
        completed: index % 3 == 0,
        feedback: vec![format!("feedback-{index}"), "检查边界".into()],
        teacher_feedback: Some(LabTeacherFeedback {
            reviewer: "teacher".into(),
            comment: "not automated feedback".into(),
            revision: 1,
            updated_at: chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
        }),
        created_at: chrono::DateTime::from_timestamp(1_700_000_000 + (index % 11) as i64, 0)
            .unwrap(),
    }
}

// Frozen previous reduction: stable timestamp sorting, first equal-time record wins.
fn legacy_progress(attempts: &[&LabAttempt]) -> Vec<LabProgress> {
    lab_definitions()
        .into_iter()
        .map(|lab| {
            let mut matching: Vec<_> = attempts
                .iter()
                .copied()
                .filter(|row| row.lab_id == lab.id)
                .collect();
            matching.sort_by_key(|row| std::cmp::Reverse(row.created_at));
            let latest = matching.first().copied();
            let completed_at = matching
                .iter()
                .filter(|row| row.completed)
                .map(|row| row.created_at)
                .min();
            LabProgress {
                lab,
                status: if completed_at.is_some() {
                    LabProgressStatus::Completed
                } else if matching.is_empty() {
                    LabProgressStatus::NotStarted
                } else {
                    LabProgressStatus::InProgress
                },
                attempts: matching.len() as u32,
                latest_stage: latest.map(|row| row.stage.clone()),
                latest_feedback: latest.map(|row| row.feedback.clone()).unwrap_or_default(),
                last_attempt_at: latest.map(|row| row.created_at),
                completed_at,
            }
        })
        .collect()
}

fn legacy_students(attempts: &[LabAttempt]) -> Value {
    let mut groups: std::collections::HashMap<&str, Vec<&LabAttempt>> =
        std::collections::HashMap::new();
    for row in attempts {
        groups.entry(&row.username).or_default().push(row);
    }
    let mut students: Vec<_> = groups
        .into_iter()
        .map(|(username, rows)| {
            let labs = legacy_progress(&rows);
            crate::models::learning::StudentLearningOverview {
                username: username.into(),
                total_labs: labs.len() as u32,
                completed_labs: labs
                    .iter()
                    .filter(|lab| lab.status == LabProgressStatus::Completed)
                    .count() as u32,
                total_attempts: rows.len() as u32,
                last_activity_at: rows.iter().map(|row| row.created_at).max(),
                labs,
            }
        })
        .collect();
    students.sort_by(|a, b| {
        b.last_activity_at
            .cmp(&a.last_activity_at)
            .then_with(|| a.username.cmp(&b.username))
    });
    json!(students)
}

#[test]
fn bounded_selection_matches_stable_sort_with_ties_and_arbitrary_input_order() {
    let mut rows: Vec<_> = (0..600).map(attempt).collect();
    for rotation in [0, 17, 421] {
        rows.rotate_left(rotation);
        for owner in ["student-0", "student-6", "missing"] {
            let matching = || rows.iter().filter(|row| row.username == owner);
            let mut sorted: Vec<_> = matching().collect();
            sorted.sort_by_key(|row| std::cmp::Reverse(row.created_at));
            for limit in [0, 1, 20, 50, 500] {
                let selected = select_recent(matching(), Some(limit));
                assert_eq!(selected.len(), sorted.len().min(limit));
                assert!(
                    selected.capacity() <= limit,
                    "bounded result storage must not grow with history"
                );
                for (actual, expected) in selected.iter().zip(&sorted) {
                    assert!(
                        std::ptr::eq(*actual, *expected),
                        "return borrowed originals in stable order"
                    );
                }
            }
            assert_eq!(json!(select_recent(matching(), None)), json!(sorted));
        }
    }
}

#[test]
fn zero_limit_does_not_scan_and_a_limited_query_visits_each_input_at_most_once() {
    let rows: Vec<_> = (0..1000).map(attempt).collect();
    let visits = std::cell::Cell::new(0);
    assert!(select_recent(
        rows.iter().inspect(|_| visits.set(visits.get() + 1)),
        Some(0)
    )
    .is_empty());
    assert_eq!(visits.get(), 0);
    let selected = select_recent(
        rows.iter().inspect(|_| visits.set(visits.get() + 1)),
        Some(20),
    );
    assert_eq!(visits.get(), rows.len());
    assert_eq!(selected.len(), 20);
    assert_eq!(selected.capacity(), 20);
}

#[test]
fn single_pass_aggregates_match_previous_progress_and_overview_semantics() {
    let mut rows: Vec<_> = (0..400).map(attempt).collect();
    let mut unknown = attempt(401);
    unknown.username = "only-unknown-lab".into();
    unknown.lab_id = "retired-lab".into();
    rows.push(unknown);
    for rotation in [0, 103, 311] {
        rows.rotate_left(rotation);
        for owner in ["student-0", "student-6", "only-unknown-lab", "missing"] {
            let matching: Vec<_> = rows.iter().filter(|row| row.username == owner).collect();
            assert_eq!(
                json!(progress_from_attempts(matching.iter().copied())),
                json!(legacy_progress(&matching))
            );
        }
        let overview = teacher_overview_from_attempts(rows.iter());
        assert_eq!(overview.active_students, 8);
        assert_eq!(overview.total_labs, 5);
        assert_eq!(json!(overview.students), legacy_students(&rows));
    }
    assert!(teacher_overview_from_attempts(std::iter::empty())
        .students
        .is_empty());
    let visits = std::cell::Cell::new(0);
    teacher_overview_from_attempts(rows.iter().inspect(|_| visits.set(visits.get() + 1)));
    assert_eq!(
        visits.get(),
        rows.len(),
        "overview must consume the input only once"
    );
}
