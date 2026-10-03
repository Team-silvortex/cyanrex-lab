use cyanrex_engine::{
    domain_packs::ebpf_teaching::{EbpfTeachingPack, LabRunEvidence},
    models::{collaboration::AssessmentVerdict, learning::LabProgressStatus},
    services::{
        learning_catalog::{assess_lab_run, find_lab, lab_definitions},
        learning_store::{LearningRunOutcome, LearningStore},
        task_catalog::{TaskCatalog, TaskCatalogError, TaskProvider},
    },
};
use serde_json::json;

const SOURCE: &str = "SEC(\"xdp\") int main(void *ctx) { return XDP_PASS; }";

#[test]
fn compatibility_catalog_preserves_all_five_legacy_definitions() {
    let expected = json!([
        {"id":"01-first-program", "position":1, "title":"Understand the eBPF execution pipeline",
         "summary":"Compile and load XDP Pass, then inspect its lifecycle.",
         "doc_slug":"labs/01-first-program", "template_id":"xdp-pass"},
        {"id":"02-trace-execve", "position":2, "title":"Observe execve with a tracepoint",
         "summary":"Attach a tracepoint program and emit a teaching trace event.",
         "doc_slug":"labs/02-trace-execve", "template_id":"tracepoint-sys-enter"},
        {"id":"03-map-counter", "position":3, "title":"Count with an eBPF map",
         "summary":"Use a per-CPU counter with a verifier-safe null check.",
         "doc_slug":"labs/03-map-counter", "template_id":"ringbuf-hi-freq-sampler"},
        {"id":"04-ring-buffer", "position":4, "title":"Send structured Ring Buffer events",
         "summary":"Reserve, populate, and submit structured kernel events.",
         "doc_slug":"labs/04-ring-buffer", "template_id":"ringbuf-skeleton"},
        {"id":"05-verifier-debugging", "position":5, "title":"Reason about verifier failures",
         "summary":"Finish with a successful verifier-safe integration run.",
         "doc_slug":"labs/05-verifier-debugging", "template_id":null}
    ]);
    assert_eq!(serde_json::to_value(lab_definitions()).unwrap(), expected);
    for lab in lab_definitions() {
        assert_eq!(find_lab(&lab.id), Some(lab));
    }
    assert!(find_lab("unknown").is_none());
}

#[test]
fn all_legacy_assessments_pass_through_versioned_definitions_without_semantic_drift() {
    let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
    assert_eq!(catalog.definitions().len(), 5);
    for lab in lab_definitions() {
        let reference = EbpfTeachingPack::definition_ref(&lab.id).unwrap();
        let definition = catalog.get(&reference).unwrap();
        assert_eq!(definition.title, lab.title);
        assert_eq!(definition.summary, lab.summary);
        for source in [
            SOURCE,
            "/* bpf_printk(fake) */",
            "SEC(\"tracepoint/syscalls/sys_enter_execve\") bpf_printk(\"ok\");",
        ] {
            for run_success in [false, true] {
                for stage in ["compile", "load", "run"] {
                    for attach_verified in [false, true] {
                        let generic = catalog
                            .assess(
                                &reference,
                                LabRunEvidence {
                                    template_id: lab.template_id.as_deref(),
                                    source,
                                    run_success,
                                    stage,
                                    attach_expected: true,
                                    attach_verified,
                                },
                            )
                            .unwrap();
                        let legacy = assess_lab_run(
                            &lab.id,
                            lab.template_id.as_deref(),
                            source,
                            run_success,
                            stage,
                            true,
                            attach_verified,
                        )
                        .unwrap();
                        assert_eq!(generic.definition, reference);
                        assert_eq!(generic.policy, definition.assessment_policy);
                        assert_eq!(
                            legacy.completed,
                            generic.outcome.verdict == AssessmentVerdict::Passed
                        );
                        assert_eq!(legacy.feedback, generic.outcome.feedback);
                    }
                }
            }
        }
    }
}

#[test]
fn legacy_feedback_text_order_and_unknown_lab_errors_remain_unchanged() {
    let rejected =
        assess_lab_run("02-trace-execve", None, "", false, "compile", false, false).unwrap();
    assert!(!rejected.completed);
    assert_eq!(rejected.feedback, vec![
        "Run did not complete successfully (stage: compile).",
        "Select the required template: tracepoint-sys-enter.",
        "Required source evidence is missing: program section SEC(\"tracepoint/syscalls/sys_enter_execve\").",
        "Required source evidence is missing: call to `bpf_printk`.",
        "The expected kernel attachment was not verified.",
    ]);
    let passed = assess_lab_run(
        "01-first-program",
        Some("xdp-pass"),
        SOURCE,
        true,
        "run",
        false,
        false,
    )
    .unwrap();
    assert!(passed.completed);
    assert_eq!(
        passed.feedback,
        vec!["Automated runtime checks passed; explanation questions still require review."]
    );
    assert_eq!(
        assess_lab_run("unknown", None, "", false, "run", false, false),
        Err("unknown lab id: unknown".into())
    );
    assert!(EbpfTeachingPack::definition_ref("unknown").is_none());
}

#[test]
fn teaching_provider_rejects_unknown_pins_instead_of_using_current_rules() {
    let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
    let mut reference = EbpfTeachingPack::definition_ref("01-first-program").unwrap();
    reference.version = 2.try_into().unwrap();
    let evidence = LabRunEvidence {
        template_id: Some("xdp-pass"),
        source: SOURCE,
        run_success: true,
        stage: "run",
        attach_expected: false,
        attach_verified: false,
    };
    assert_eq!(
        catalog.assess(&reference, evidence),
        Err(TaskCatalogError::UnknownDefinition)
    );
}

#[test]
fn direct_provider_calls_cannot_substitute_policy_or_schema_metadata() {
    let pack = EbpfTeachingPack;
    for field in 0..4 {
        let mut definition = pack.definitions().remove(0);
        match field {
            0 => definition.assessment_policy.version = 2.try_into().unwrap(),
            1 => definition.evidence_schema.version = 2.try_into().unwrap(),
            2 => definition.reference.package.version = 2.try_into().unwrap(),
            _ => definition.reference.name = "example.documents.summary".parse().unwrap(),
        }
        let evidence = LabRunEvidence {
            template_id: Some("xdp-pass"),
            source: SOURCE,
            run_success: true,
            stage: "run",
            attach_expected: false,
            attach_verified: false,
        };
        assert_eq!(
            pack.assess(&definition, evidence),
            Err(TaskCatalogError::UnknownDefinition)
        );
    }
}

#[tokio::test]
async fn live_store_preserves_history_and_first_completion_across_reload() {
    let root = std::env::temp_dir().join(format!("cyanrex-task-adapter-{}", uuid::Uuid::new_v4()));
    let path = root.join("attempts.json");
    let store = LearningStore::with_local_data_path(path.clone());
    let outcome = |success| LearningRunOutcome {
        lab_id: "01-first-program",
        template_id: Some("xdp-pass"),
        source: SOURCE,
        run_success: success,
        stage: "run",
        attach_expected: false,
        attach_verified: false,
    };
    let passed = store
        .record_run("task-student", outcome(true))
        .await
        .unwrap();
    let failed = store
        .record_run("task-student", outcome(false))
        .await
        .unwrap();
    assert!(passed.completed);
    assert!(!failed.completed);
    assert_ne!(passed.id, failed.id);
    let reloaded = LearningStore::with_local_data_path(path);
    let progress = reloaded.progress_for_user("task-student").await.unwrap();
    assert_eq!(progress[0].status, LabProgressStatus::Completed);
    assert_eq!(progress[0].completed_at, Some(passed.created_at));
    assert_eq!(progress[0].attempts, 2);
    let restored = reloaded
        .attempt_for_user("task-student", &passed.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(passed).unwrap()
    );
    assert!(reloaded
        .attempt_for_user("other-student", &failed.id)
        .await
        .unwrap()
        .is_none());
    std::fs::remove_dir_all(root).unwrap();
}
