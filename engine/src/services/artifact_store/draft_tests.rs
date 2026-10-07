use crate::services::task_draft_import::TaskDraftPublicationPlan;

#[test]
fn task_publication_draft_preserves_fixed_metadata_and_exact_bytes_without_io() {
    let title = "  Named Task 📝  ";
    let text = "中文 👩‍💻 e\u{301}\t\r\n<script>inert text</script>";
    let input = serde_json::json!({
        "format": "cyanrex.task-draft", "version": 1, "title": title,
        "payload": [{
            "id": "local_only", "revision": 7, "kind": "text",
            "filename": "中".repeat(128), "language": "future-lang", "text": text,
        }],
    });
    let bytes = serde_json::to_vec(&input).unwrap();
    let plan = TaskDraftPublicationPlan::parse_json(&bytes).unwrap();
    let draft = plan.publication_draft(0).unwrap();
    // This owning module can inspect the private draft without adding public getters or storage.
    assert_eq!(draft.kind.as_str(), "cyanrex.task-text");
    assert_eq!(draft.title, title);
    assert_eq!(draft.media_type, "text/plain");
    assert_eq!(draft.bytes, text.as_bytes());
    assert_eq!(plan.items()[0].text(), text);
}
