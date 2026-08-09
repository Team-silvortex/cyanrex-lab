use crate::models::learning::LabDefinition;

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
    required_source_tokens: &'static [&'static str],
    require_verified_attach: bool,
}

const LAB_RULES: &[LabRule] = &[
    LabRule {
        id: "01-first-program",
        position: 1,
        title: "Understand the eBPF execution pipeline",
        summary: "Compile and load XDP Pass, then inspect its lifecycle.",
        template_id: Some("xdp-pass"),
        required_source_tokens: &["SEC(\"xdp\")", "XDP_PASS"],
        require_verified_attach: false,
    },
    LabRule {
        id: "02-trace-execve",
        position: 2,
        title: "Observe execve with a tracepoint",
        summary: "Attach a tracepoint program and emit a teaching trace event.",
        template_id: Some("tracepoint-sys-enter"),
        required_source_tokens: &["sys_enter_execve", "bpf_printk"],
        require_verified_attach: true,
    },
    LabRule {
        id: "03-map-counter",
        position: 3,
        title: "Count with an eBPF map",
        summary: "Use a per-CPU counter with a verifier-safe null check.",
        template_id: Some("ringbuf-hi-freq-sampler"),
        required_source_tokens: &["per_cpu_counter", "bpf_map_lookup_elem", "if (!counter)"],
        require_verified_attach: true,
    },
    LabRule {
        id: "04-ring-buffer",
        position: 4,
        title: "Send structured Ring Buffer events",
        summary: "Reserve, populate, and submit structured kernel events.",
        template_id: Some("ringbuf-skeleton"),
        required_source_tokens: &["bpf_ringbuf_reserve", "bpf_ringbuf_submit"],
        require_verified_attach: true,
    },
    LabRule {
        id: "05-verifier-debugging",
        position: 5,
        title: "Reason about verifier failures",
        summary: "Finish with a successful verifier-safe integration run.",
        template_id: None,
        required_source_tokens: &["SEC("],
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
    }
    if let Some(expected) = rule.template_id {
        if template_id != Some(expected) {
            feedback.push(format!("Select the required template: {expected}."));
        }
    }
    for token in rule.required_source_tokens {
        if !source.contains(token) {
            feedback.push(format!("Required source pattern is missing: {token}."));
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
}
