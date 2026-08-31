fn default_templates() -> Vec<EbpfTemplate> {
    let mut templates = base_templates();
    templates.extend(extra_templates());
    templates.extend(mark_template_category(
        learning_templates(),
        "learning/foundations/beginner/fundamentals",
    ));
    templates.extend(mark_template_category(
        learning_plus_templates(),
        "learning/foundations/intermediate/protocols",
    ));
    templates.extend(mark_template_category(
        learning_plus_two_templates(),
        "learning-plus/cases/advanced/forensics",
    ));
    templates.extend(mark_template_category(
        learning_plus_three_templates(),
        "learning-plus/track/practice/operators",
    ));
    templates
}

#[cfg(test)]
mod learning_template_assessment_tests {
    use super::base_templates;
    use crate::services::learning_catalog::{assess_lab_run, lab_definitions};

    #[test]
    fn bundled_beginner_templates_satisfy_structural_assessment() {
        let templates = base_templates();

        for lab in lab_definitions() {
            let template_id = lab.template_id.as_deref().unwrap_or("xdp-pass");
            let template = templates
                .iter()
                .find(|candidate| candidate.id == template_id)
                .unwrap_or_else(|| panic!("missing bundled template: {template_id}"));
            let assessment = assess_lab_run(
                &lab.id,
                Some(template_id),
                &template.code,
                true,
                "run",
                true,
                true,
            )
            .unwrap_or_else(|error| panic!("assessment failed for {}: {error}", lab.id));

            assert!(
                assessment.completed,
                "bundled template {template_id} failed {}: {:?}",
                lab.id, assessment.feedback
            );
        }
    }
}
