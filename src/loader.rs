use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::error::{LoadError, ParseError};
use crate::model::Snapshot;

#[derive(Clone, Debug)]
pub struct SourceState {
    pub modified_at: SystemTime,
    pub checksum: blake3::Hash,
}

#[derive(Debug)]
pub enum ReadOutcome {
    Loaded {
        snapshot: Snapshot,
        source_state: SourceState,
    },
    Unchanged {
        source_state: SourceState,
    },
    Rejected {
        error: ParseError,
        source_state: SourceState,
    },
}

pub fn load_tasks_text(path: &Path) -> Result<String, LoadError> {
    validate_source_path(path)?;
    let bytes = fs::read(path).map_err(|source| LoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    String::from_utf8(bytes).map_err(|source| LoadError::InvalidUtf8 {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_snapshot(
    path: &Path,
    previous_state: Option<&SourceState>,
) -> Result<ReadOutcome, LoadError> {
    validate_source_path(path)?;

    let metadata = fs::metadata(path).map_err(|source| LoadError::Metadata {
        path: path.to_path_buf(),
        source,
    })?;
    let modified_at = metadata
        .modified()
        .map_err(|source| LoadError::ModifiedTime {
            path: path.to_path_buf(),
            source,
        })?;

    if let Some(previous_state) = previous_state {
        if modified_at == previous_state.modified_at {
            return Ok(ReadOutcome::Unchanged {
                source_state: previous_state.clone(),
            });
        }
    }

    let text = load_tasks_text(path)?;
    let checksum = blake3::hash(text.as_bytes());

    let source_state = SourceState {
        modified_at,
        checksum,
    };

    if previous_state.is_some_and(|previous_state| source_state.checksum == previous_state.checksum)
    {
        return Ok(ReadOutcome::Unchanged { source_state });
    }

    match Snapshot::from_yaml_str(&text) {
        Ok(snapshot) => Ok(ReadOutcome::Loaded {
            snapshot,
            source_state,
        }),
        Err(error) if previous_state.is_some() => Ok(ReadOutcome::Rejected {
            error,
            source_state,
        }),
        Err(error) => Err(LoadError::Parse(error)),
    }
}

fn validate_source_path(path: &Path) -> Result<(), LoadError> {
    validate_yaml_extension(path)?;

    let symlink_metadata = fs::symlink_metadata(path).map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => LoadError::MissingPath {
            path: path.to_path_buf(),
        },
        _ => LoadError::Metadata {
            path: path.to_path_buf(),
            source,
        },
    })?;

    if symlink_metadata.file_type().is_symlink() && !path.exists() {
        return Err(LoadError::BrokenSymlink {
            path: path.to_path_buf(),
        });
    }

    if symlink_metadata.is_dir() {
        return Err(LoadError::DirectoryPath {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}

fn validate_yaml_extension(path: &Path) -> Result<(), LoadError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("yaml") | Some("yml") => Ok(()),
        _ => Err(LoadError::InvalidExtension {
            path: path.to_path_buf(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;

    const VALID_YAML: &str = r#"
schema_version: 1
captured_on: 2026-03-17
source_files: []
ingestion_rules: []
tasks:
  p1:
    - id: p1-001
      rank: 1
      status: todo
      title: Priority Item Alpha
      raw_text: Priority Item Alpha
      links: []
      notes: []
  p2: []
  p3: []
dailies:
  active: []
  later: []
session_state:
  active_work: []
  blocked: []
  daily_logs: []
decisions: []
"#;

    #[test]
    fn load_tasks_text_rejects_wrong_extension() {
        let path = PathBuf::from("/tmp/tasks.txt");
        let err = load_tasks_text(&path).expect_err("wrong extension should fail");
        assert!(matches!(err, LoadError::InvalidExtension { .. }));
    }

    #[test]
    fn load_tasks_text_rejects_missing_file() {
        let path = unique_temp_path("missing.yaml");
        let err = load_tasks_text(&path).expect_err("missing file should fail");
        assert!(matches!(err, LoadError::MissingPath { .. }));
    }

    #[test]
    fn load_tasks_text_rejects_directory() {
        let dir = unique_temp_path("dir.yaml");
        fs::create_dir_all(&dir).expect("directory should be created");

        let err = load_tasks_text(&dir).expect_err("directory path should fail");
        assert!(matches!(err, LoadError::DirectoryPath { .. }));

        fs::remove_dir_all(&dir).expect("directory should be removed");
    }

    #[test]
    fn load_tasks_text_rejects_broken_symlink() {
        let link = unique_temp_path("broken.yaml");
        let target = unique_temp_path("missing-target.yaml");

        std::os::unix::fs::symlink(&target, &link).expect("broken symlink should be created");

        let err = load_tasks_text(&link).expect_err("broken symlink should fail");
        assert!(matches!(err, LoadError::BrokenSymlink { .. }));

        fs::remove_file(&link).expect("symlink should be removed");
    }

    #[test]
    fn read_snapshot_loads_initial_state() {
        let path = write_temp_yaml("initial.yaml", VALID_YAML);

        let outcome = read_snapshot(&path, None).expect("initial load should succeed");
        let ReadOutcome::Loaded { snapshot, .. } = outcome else {
            panic!("expected loaded state");
        };

        assert_eq!(snapshot.schema_version, 1);

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn read_snapshot_skips_when_timestamp_is_unchanged() {
        let path = write_temp_yaml("unchanged.yaml", VALID_YAML);
        let ReadOutcome::Loaded {
            source_state: initial_state,
            ..
        } = read_snapshot(&path, None).expect("initial load should succeed")
        else {
            panic!("expected loaded state");
        };

        let outcome =
            read_snapshot(&path, Some(&initial_state)).expect("second read should succeed");
        assert!(matches!(outcome, ReadOutcome::Unchanged { .. }));

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn read_snapshot_skips_parse_when_timestamp_changes_but_content_does_not() {
        let path = write_temp_yaml("same-bytes.yaml", VALID_YAML);
        let ReadOutcome::Loaded {
            source_state: initial_state,
            ..
        } = read_snapshot(&path, None).expect("initial load should succeed")
        else {
            panic!("expected loaded state");
        };

        wait_for_timestamp_tick();
        fs::write(&path, VALID_YAML).expect("temp file should be rewritten");

        let outcome =
            read_snapshot(&path, Some(&initial_state)).expect("second read should succeed");
        match outcome {
            ReadOutcome::Unchanged { source_state } => {
                assert!(source_state.modified_at > initial_state.modified_at)
            }
            _ => panic!("expected unchanged outcome"),
        }

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn read_snapshot_reloads_when_content_changes() {
        let path = write_temp_yaml("changed.yaml", VALID_YAML);
        let ReadOutcome::Loaded {
            source_state: initial_state,
            ..
        } = read_snapshot(&path, None).expect("initial load should succeed")
        else {
            panic!("expected loaded state");
        };

        let changed_yaml = VALID_YAML.replace("Priority Item Alpha", "Priority Item Beta");
        wait_for_timestamp_tick();
        fs::write(&path, changed_yaml).expect("temp file should be rewritten");

        let outcome = read_snapshot(&path, Some(&initial_state)).expect("reload should succeed");
        let ReadOutcome::Loaded {
            snapshot,
            source_state,
        } = outcome
        else {
            panic!("expected reloaded state");
        };

        assert_eq!(snapshot.tasks.p1[0].title, "Priority Item Beta");
        assert_ne!(source_state.checksum, initial_state.checksum);

        fs::remove_file(path).expect("temp file should be removed");
    }

    #[test]
    fn read_snapshot_preserves_last_good_state_on_invalid_reload() {
        let path = write_temp_yaml("invalid-reload.yaml", VALID_YAML);
        let ReadOutcome::Loaded {
            snapshot: initial_snapshot,
            source_state: initial_state,
        } = read_snapshot(&path, None).expect("initial load should succeed")
        else {
            panic!("expected loaded state");
        };

        wait_for_timestamp_tick();
        fs::write(&path, "schema_version: [").expect("temp file should be rewritten");

        let outcome =
            read_snapshot(&path, Some(&initial_state)).expect("reload check should succeed");
        match outcome {
            ReadOutcome::Rejected { error, .. } => {
                assert!(matches!(error, ParseError::InvalidYaml(_)));
                assert_eq!(initial_snapshot.tasks.p1[0].title, "Priority Item Alpha");
            }
            _ => panic!("expected rejected reload"),
        }

        fs::remove_file(path).expect("temp file should be removed");
    }

    fn unique_temp_path(file_name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        path.push(format!("learning-computer-{nanos}-{file_name}"));
        path
    }

    fn write_temp_yaml(file_name: &str, contents: &str) -> PathBuf {
        let path = unique_temp_path(file_name);
        let mut file = File::create(&path).expect("temp file should be created");
        file.write_all(contents.as_bytes())
            .expect("temp file should be written");
        path
    }

    fn wait_for_timestamp_tick() {
        thread::sleep(Duration::from_millis(1100));
    }
}
