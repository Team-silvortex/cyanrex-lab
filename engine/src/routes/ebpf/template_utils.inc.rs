fn mark_template_category(mut templates: Vec<EbpfTemplate>, category: &str) -> Vec<EbpfTemplate> {
    for template in &mut templates {
        template.category = Some(category.to_string());
    }
    templates
}
