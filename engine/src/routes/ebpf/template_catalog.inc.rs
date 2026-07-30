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
