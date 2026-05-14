use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use codex_apply_patch::AppliedPatchChange;
use codex_apply_patch::AppliedPatchDelta;
use codex_apply_patch::AppliedPatchFileChange;
use serde::Deserialize;
use serde::Serialize;
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileRestoreSummary {
    pub(crate) restored_files: usize,
    pub(crate) removed_files: usize,
}

#[derive(Debug)]
pub(crate) struct FileHistory {
    state_path: PathBuf,
    state: FileHistoryState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FileHistoryState {
    snapshots: Vec<FileHistorySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileHistorySnapshot {
    turn_id: String,
    cwd: PathBuf,
    backups: BTreeMap<PathBuf, FileBackup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum FileBackup {
    Missing,
    Present { content: String },
}

impl FileHistory {
    pub(crate) fn new(state_path: PathBuf) -> Self {
        Self {
            state_path,
            state: FileHistoryState::default(),
        }
    }

    pub(crate) fn load_or_new(state_path: PathBuf) -> anyhow::Result<Self> {
        let state = match fs::read_to_string(&state_path) {
            Ok(content) => serde_json::from_str(&content)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => FileHistoryState::default(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self { state_path, state })
    }

    pub(crate) fn begin_turn(&mut self, turn_id: String, cwd: PathBuf) -> anyhow::Result<()> {
        if self
            .state
            .snapshots
            .last()
            .is_some_and(|snapshot| snapshot.turn_id == turn_id)
        {
            return Ok(());
        }
        self.state.snapshots.push(FileHistorySnapshot {
            turn_id,
            cwd,
            backups: BTreeMap::new(),
        });
        self.persist()
    }

    pub(crate) fn track_delta(&mut self, delta: &AppliedPatchDelta) -> anyhow::Result<()> {
        if !delta.is_exact() || delta.is_empty() {
            return Ok(());
        }
        self.track_changes(delta.changes())
    }

    #[cfg(test)]
    fn track_changes_for_test(&mut self, changes: Vec<AppliedPatchChange>) -> anyhow::Result<()> {
        self.track_changes(&changes)
    }

    fn track_changes(&mut self, changes: &[AppliedPatchChange]) -> anyhow::Result<()> {
        let Some(snapshot) = self.state.snapshots.last_mut() else {
            return Ok(());
        };

        for change in changes {
            match &change.change {
                AppliedPatchFileChange::Add {
                    overwritten_content,
                    ..
                } => {
                    record_backup(
                        snapshot,
                        &change.path,
                        backup_from_optional_content(overwritten_content.as_deref()),
                    );
                }
                AppliedPatchFileChange::Delete { content } => {
                    record_backup(
                        snapshot,
                        &change.path,
                        FileBackup::Present {
                            content: content.clone(),
                        },
                    );
                }
                AppliedPatchFileChange::Update {
                    move_path,
                    old_content,
                    overwritten_move_content,
                    ..
                } => {
                    record_backup(
                        snapshot,
                        &change.path,
                        FileBackup::Present {
                            content: old_content.clone(),
                        },
                    );
                    if let Some(move_path) = move_path {
                        record_backup(
                            snapshot,
                            move_path,
                            backup_from_optional_content(overwritten_move_content.as_deref()),
                        );
                    }
                }
            }
        }

        self.persist()
    }

    pub(crate) fn restore_before_last_n_turns(
        &mut self,
        num_turns: u32,
    ) -> anyhow::Result<FileRestoreSummary> {
        if num_turns == 0 || self.state.snapshots.is_empty() {
            return Ok(FileRestoreSummary {
                restored_files: 0,
                removed_files: 0,
            });
        }

        let turns_from_end = usize::try_from(num_turns).unwrap_or(usize::MAX);
        let target_index = self
            .state
            .snapshots
            .len()
            .saturating_sub(turns_from_end)
            .min(self.state.snapshots.len().saturating_sub(1));
        let snapshot = self.state.snapshots[target_index].clone();
        let summary = restore_snapshot(&snapshot)?;
        self.state.snapshots.truncate(target_index);
        self.persist()?;
        Ok(summary)
    }

    fn persist(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.state)?;
        fs::write(&self.state_path, content)?;
        Ok(())
    }
}

fn backup_from_optional_content(content: Option<&str>) -> FileBackup {
    match content {
        Some(content) => FileBackup::Present {
            content: content.to_string(),
        },
        None => FileBackup::Missing,
    }
}

fn record_backup(snapshot: &mut FileHistorySnapshot, path: &Path, backup: FileBackup) {
    snapshot.backups.entry(path.to_path_buf()).or_insert(backup);
}

fn restore_snapshot(snapshot: &FileHistorySnapshot) -> anyhow::Result<FileRestoreSummary> {
    let mut summary = FileRestoreSummary {
        restored_files: 0,
        removed_files: 0,
    };

    for (path, backup) in &snapshot.backups {
        match backup {
            FileBackup::Missing => {
                if path.exists() {
                    fs::remove_file(path)?;
                    summary.removed_files += 1;
                }
            }
            FileBackup::Present { content } => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(path, content)?;
                summary.restored_files += 1;
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_apply_patch::AppliedPatchChange;
    use codex_apply_patch::AppliedPatchFileChange;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn restore_reverts_add_update_delete_and_move_to_checkpoint_state() {
        let temp = TempDir::new().expect("tempdir");
        let root = temp.path();
        let updated = root.join("updated.txt");
        let deleted = root.join("deleted.txt");
        let moved_from = root.join("moved-from.txt");
        let moved_to = root.join("moved-to.txt");
        std::fs::write(&updated, "before update\n").expect("write updated");
        std::fs::write(&deleted, "before delete\n").expect("write deleted");
        std::fs::write(&moved_from, "before move\n").expect("write move source");
        std::fs::write(&moved_to, "preexisting dest\n").expect("write move dest");

        let mut history = FileHistory::new(temp.path().join("state.json"));
        history
            .begin_turn("turn-1".to_string(), root.to_path_buf())
            .expect("begin turn");

        std::fs::write(&updated, "after update\n").expect("mutate update");
        std::fs::write(root.join("added.txt"), "new file\n").expect("mutate add");
        std::fs::remove_file(&deleted).expect("mutate delete");
        std::fs::write(&moved_to, "after move\n").expect("mutate move dest");
        std::fs::remove_file(&moved_from).expect("mutate move source");

        history
            .track_changes_for_test(vec![
                AppliedPatchChange {
                    path: updated.clone(),
                    change: AppliedPatchFileChange::Update {
                        move_path: None,
                        old_content: "before update\n".to_string(),
                        overwritten_move_content: None,
                        new_content: "after update\n".to_string(),
                    },
                },
                AppliedPatchChange {
                    path: root.join("added.txt"),
                    change: AppliedPatchFileChange::Add {
                        content: "new file\n".to_string(),
                        overwritten_content: None,
                    },
                },
                AppliedPatchChange {
                    path: deleted.clone(),
                    change: AppliedPatchFileChange::Delete {
                        content: "before delete\n".to_string(),
                    },
                },
                AppliedPatchChange {
                    path: moved_from.clone(),
                    change: AppliedPatchFileChange::Update {
                        move_path: Some(moved_to.clone()),
                        old_content: "before move\n".to_string(),
                        overwritten_move_content: Some("preexisting dest\n".to_string()),
                        new_content: "after move\n".to_string(),
                    },
                },
            ])
            .expect("track delta");

        let restored = history
            .restore_before_last_n_turns(/*num_turns*/ 1)
            .expect("restore");

        assert_eq!(
            restored,
            FileRestoreSummary {
                restored_files: 4,
                removed_files: 1,
            }
        );
        assert_eq!(
            std::fs::read_to_string(&updated).expect("read updated"),
            "before update\n"
        );
        assert_eq!(
            std::fs::read_to_string(&deleted).expect("read deleted"),
            "before delete\n"
        );
        assert_eq!(
            std::fs::read_to_string(&moved_from).expect("read moved source"),
            "before move\n"
        );
        assert_eq!(
            std::fs::read_to_string(&moved_to).expect("read moved dest"),
            "preexisting dest\n"
        );
        assert!(!root.join("added.txt").exists());
    }

    #[tokio::test]
    async fn track_delta_keeps_first_baseline_for_repeated_file_changes() {
        let temp = TempDir::new().expect("tempdir");
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "one\n").expect("write file");

        let mut history = FileHistory::new(temp.path().join("state.json"));
        history
            .begin_turn("turn-1".to_string(), temp.path().to_path_buf())
            .expect("begin turn");

        std::fs::write(&file, "two\n").expect("first mutation");
        history
            .track_changes_for_test(vec![AppliedPatchChange {
                path: file.clone(),
                change: AppliedPatchFileChange::Update {
                    move_path: None,
                    old_content: "one\n".to_string(),
                    overwritten_move_content: None,
                    new_content: "two\n".to_string(),
                },
            }])
            .expect("track first delta");

        std::fs::write(&file, "three\n").expect("second mutation");
        history
            .track_changes_for_test(vec![AppliedPatchChange {
                path: file.clone(),
                change: AppliedPatchFileChange::Update {
                    move_path: None,
                    old_content: "two\n".to_string(),
                    overwritten_move_content: None,
                    new_content: "three\n".to_string(),
                },
            }])
            .expect("track second delta");

        history
            .restore_before_last_n_turns(/*num_turns*/ 1)
            .expect("restore");

        assert_eq!(
            std::fs::read_to_string(&file).expect("read restored file"),
            "one\n"
        );
    }
}
