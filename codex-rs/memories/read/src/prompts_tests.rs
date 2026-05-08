use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::fs as tokio_fs;

#[tokio::test]
async fn build_memory_tool_developer_instructions_renders_embedded_template() {
    let temp = tempdir().unwrap();
    let codex_home = AbsolutePathBuf::from_absolute_path(temp.path()).unwrap();
    let memories_dir = codex_home.join("memories");
    tokio_fs::create_dir_all(&memories_dir).await.unwrap();
    tokio_fs::write(
        memories_dir.join("memory_summary.md"),
        "Short memory summary for tests.",
    )
    .await
    .unwrap();

    let instructions = build_memory_tool_developer_instructions(&codex_home)
        .await
        .unwrap();

    assert!(instructions.contains(&format!(
        "- {}/memory_summary.md (already provided below; do NOT open again)",
        memories_dir.display()
    )));
    assert!(instructions.contains("Short memory summary for tests."));
    assert_eq!(
        instructions
            .matches("========= MEMORY_SUMMARY BEGINS =========")
            .count(),
        1
    );
}

#[tokio::test]
async fn build_memory_tool_developer_instructions_can_include_session_overlay() {
    let temp = tempdir().unwrap();
    let codex_home = AbsolutePathBuf::from_absolute_path(temp.path()).unwrap();
    let memories_dir = codex_home.join("memories");
    tokio_fs::create_dir_all(&memories_dir).await.unwrap();
    tokio_fs::write(memories_dir.join("memory_summary.md"), "Durable summary.")
        .await
        .unwrap();

    let instructions = build_memory_tool_developer_instructions_with_session_overlay(
        &codex_home,
        Some("Prefer the active overlay in this session."),
    )
    .await
    .unwrap();

    assert!(instructions.contains("Durable summary."));
    assert!(instructions.contains("## Session Memory Overlay"));
    assert!(instructions.contains("Prefer the active overlay in this session."));
}

#[tokio::test]
async fn build_memory_tool_developer_instructions_allows_overlay_without_summary() {
    let temp = tempdir().unwrap();
    let codex_home = AbsolutePathBuf::from_absolute_path(temp.path()).unwrap();
    tokio_fs::create_dir_all(codex_home.join("memories"))
        .await
        .unwrap();

    let instructions = build_memory_tool_developer_instructions_with_session_overlay(
        &codex_home,
        Some("Freshly staged memory."),
    )
    .await
    .unwrap();

    assert!(instructions.contains("## Session Memory Overlay"));
    assert!(instructions.contains("Freshly staged memory."));
}
