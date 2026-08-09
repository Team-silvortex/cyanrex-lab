use crate::models::learning::LabDefinition;
use crate::services::learning_source::SourceEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabAssessment {
    pub completed: bool,
    pub feedback: Vec<String>,
}

struct LabRule {
    id: &'static str,
    position: u8,
    title: &'static str,
    summary: &'static str,
    template_id: Option<&'static str>,
    source_requirements: &'static [SourceRequirement],
    require_verified_attach: bool,
}

#[derive(Clone, Copy)]
enum SourceRequirement {
    Section(&'static str),
    ProgramSection,
    Identifier(&'static str),
    Call(&'static str),
    ReturnIdentifier(&'static str),
    AssignedCall {
        variable: &'static str,
        function: &'static str,
    },
    NullGuard(&'static str),
}

impl SourceRequirement {
    fn evaluate(self, evidence: &SourceEvidence) -> bool {
        match self {
            Self::Section(section) => evidence.has_section(section),
            Self::ProgramSection => evidence.has_program_section(),
            Self::Identifier(identifier) => evidence.has_identifier(identifier),
            Self::Call(function) => evidence.has_call(function),
            Self::ReturnIdentifier(identifier) => evidence.returns_identifier(identifier),
            Self::AssignedCall { variable, function } => evidence.assigns_call(variable, function),
            Self::NullGuard(variable) => evidence.has_null_guard(variable),
        }
    }

    fn description(self) -> String {
        match self {
            Self::Section(section) => format!("program section SEC(\"{section}\")"),
            Self::ProgramSection => "a non-map eBPF program section".to_string(),
            Self::Identifier(identifier) => format!("identifier `{identifier}`"),
            Self::Call(function) => format!("call to `{function}`"),
            Self::ReturnIdentifier(identifier) => format!("return of `{identifier}`"),
            Self::AssignedCall { variable, function } => {
                format!("assignment of `{function}` result to `{variable}`")
            }
            Self::NullGuard(variable) => format!("null guard for `{variable}`"),
        }
    }
}

const LAB_RULES: &[LabRule] = &[
    LabRule {
        id: "01-first-program",
        position: 1,
        title: "Understand the eBPF execution pipeline",
        summary: "Compile and load XDP Pass, then inspect its lifecycle.",
        template_id: Some("xdp-pass"),
        source_requirements: &[
            SourceRequirement::Section("xdp"),
            SourceRequirement::ReturnIdentifier("XDP_PASS"),
        ],
        require_verified_attach: false,
    },
    LabRule {
        id: "02-trace-execve",
        position: 2,
        title: "Observe execve with a tracepoint",
        summary: "Attach a tracepoint program and emit a teaching trace event.",
        template_id: Some("tracepoint-sys-enter"),
        source_requirements: &[
            SourceRequirement::Section("tracepoint/syscalls/sys_enter_execve"),
            SourceRequirement::Call("bpf_printk"),
        ],
        require_verified_attach: true,
    },
    LabRule {
        id: "03-map-counter",
        position: 3,
        title: "Count with an eBPF map",
        summary: "Use a per-CPU counter with a verifier-safe null check.",
        template_id: Some("ringbuf-hi-freq-sampler"),
        source_requirements: &[
            SourceRequirement::Identifier("per_cpu_counter"),
            SourceRequirement::AssignedCall {
                variable: "counter",
                function: "bpf_map_lookup_elem",
            },
            SourceRequirement::NullGuard("counter"),
        ],
        require_verified_attach: true,
    },
    LabRule {
        id: "04-ring-buffer",
        position: 4,
        title: "Send structured Ring Buffer events",
        summary: "Reserve, populate, and submit structured kernel events.",
        template_id: Some("ringbuf-skeleton"),
        source_requirements: &[
            SourceRequirement::Call("bpf_ringbuf_reserve"),
            SourceRequirement::Call("bpf_ringbuf_submit"),
        ],
        require_verified_attach: true,
    },
    LabRule {
        id: "05-verifier-debugging",
        position: 5,
        title: "Reason about verifier failures",
        summary: "Finish with a successful verifier-safe integration run.",
        template_id: None,
        source_requirements: &[SourceRequirement::ProgramSection],
        require_verified_attach: false,
    },
];

pub fn lab_definitions() -> Vec<LabDefinition> {
    LAB_RULES.iter().map(to_definition).collect()
}

pub fn find_lab(lab_id: &str) -> Option<LabDefinition> {
    LAB_RULES
        .iter()
        .find(|rule| rule.id == lab_id)
        .map(to_definition)
}

pub fn assess_lab_run(
    lab_id: &str,
    template_id: Option<&str>,
    source: &str,
    run_success: bool,
    stage: &str,
    attach_expected: bool,
    attach_verified: bool,
) -> Result<LabAssessment, String> {
    let rule = LAB_RULES
        .iter()
        .find(|rule| rule.id == lab_id)
        .ok_or_else(|| format!("unknown lab id: {lab_id}"))?;
    let mut feedback = Vec::new();

    if !run_success {
        feedback.push(format!(
            "Run did not complete successfully (stage: {stage})."
        ));
    } else if stage != "run" {
        feedback.push(format!(
            "Execution stopped before runtime completion (stage: {stage})."
        ));
    }
    if let Some(expected) = rule.template_id {
        if template_id != Some(expected) {
            feedback.push(format!("Select the required template: {expected}."));
        }
    }
    let evidence = SourceEvidence::parse(source);
    for requirement in rule.source_requirements {
        if !requirement.evaluate(&evidence) {
            feedback.push(format!(
                "Required source evidence is missing: {}.",
                requirement.description()
            ));
        }
    }
    if rule.require_verified_attach && (!attach_expected || !attach_verified) {
        feedback.push("The expected kernel attachment was not verified.".to_string());
    }

    let completed = feedback.is_empty();
    if completed {
        feedback.push(
            "Automated runtime checks passed; explanation questions still require review."
                .to_string(),
        );
    }
    Ok(LabAssessment {
        completed,
        feedback,
    })
}

fn to_definition(rule: &LabRule) -> LabDefinition {
    LabDefinition {
        id: rule.id.to_string(),
        position: rule.position,
        title: rule.title.to_string(),
        summary: rule.summary.to_string(),
        doc_slug: format!("labs/{}", rule.id),
        template_id: rule.template_id.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::{assess_lab_run, lab_definitions};

    #[test]
    fn catalog_has_ordered_unique_labs() {
        let labs = lab_definitions();
        assert_eq!(labs.len(), 5);
        assert!(labs
            .windows(2)
            .all(|pair| pair[0].position < pair[1].position));
        let mut ids = labs.iter().map(|lab| &lab.id).collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), labs.len());
    }

    #[test]
    fn tracepoint_lab_requires_real_success_template_and_attach() {
        let source = "SEC(\"tracepoint/syscalls/sys_enter_execve\") bpf_printk(\"ok\");";
        let passed = assess_lab_run(
            "02-trace-execve",
            Some("tracepoint-sys-enter"),
            source,
            true,
            "run",
            true,
            true,
        )
        .unwrap();
        assert!(passed.completed);

        let missing_attach = assess_lab_run(
            "02-trace-execve",
            Some("tracepoint-sys-enter"),
            source,
            true,
            "run",
            true,
            false,
        )
        .unwrap();
        assert!(!missing_attach.completed);
    }

    #[test]
    fn comments_and_strings_do_not_satisfy_source_requirements() {
        let source = r#"
// SEC("tracepoint/syscalls/sys_enter_execve")
// bpf_printk("this line is commented out");
int noop(void *ctx) {
    const char *decoy = "sys_enter_execve bpf_printk";
    return 0;
}
"#;
        let assessment = assess_lab_run(
            "02-trace-execve",
            Some("tracepoint-sys-enter"),
            source,
            true,
            "run",
            true,
            true,
        )
        .unwrap();

        assert!(!assessment.completed);
        assert!(assessment.feedback.len() >= 2);
    }

    #[test]
    fn helper_name_substrings_do_not_satisfy_call_requirements() {
        let source = r#"
SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
    fake_bpf_ringbuf_reserve_helper();
    const char *decoy = "bpf_ringbuf_submit(evt, 0)";
    return 0;
}
"#;
        let assessment = assess_lab_run(
            "04-ring-buffer",
            Some("ringbuf-skeleton"),
            source,
            true,
            "run",
            true,
            true,
        )
        .unwrap();

        assert!(!assessment.completed);
        assert!(assessment.feedback.len() >= 2);
    }

    #[test]
    fn equivalent_null_guard_is_accepted_for_map_lab() {
        let source = r#"
struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
} per_cpu_counter SEC(".maps");

SEC("tracepoint/sched/sched_switch")
int on_switch(void *ctx) {
    __u32 key = 0;
    __u64 *counter = bpf_map_lookup_elem(&per_cpu_counter, &key);
    if (counter == NULL) {
        return 0;
    }
    *counter += 1;
    return 0;
}
"#;
        let assessment = assess_lab_run(
            "03-map-counter",
            Some("ringbuf-hi-freq-sampler"),
            source,
            true,
            "run",
            true,
            true,
        )
        .unwrap();

        assert!(assessment.completed, "{:?}", assessment.feedback);
    }

    #[test]
    fn success_flag_without_runtime_stage_does_not_complete_lab() {
        let source = r#"
SEC("xdp")
int xdp_pass(void *ctx) {
    return XDP_PASS;
}
"#;
        let assessment = assess_lab_run(
            "01-first-program",
            Some("xdp-pass"),
            source,
            true,
            "compile",
            false,
            false,
        )
        .unwrap();

        assert!(!assessment.completed);
        assert!(assessment.feedback[0].contains("before runtime completion"));
    }
}
