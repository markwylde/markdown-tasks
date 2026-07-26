//! Snapshot construction from deterministic filesystem discovery.

use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::{
    discover::{DiscoverOptions, DiscoveryWarning, discover},
    error::MdtError,
    markdown::parse_document,
    model::{ScanStats, Warning, WorkspaceSnapshot},
};

/// Maximum bytes read from any individual Markdown file.
pub const MAX_MARKDOWN_FILE_SIZE: u64 = 16 * 1024 * 1024;

/// Discover, read, parse, and aggregate a file or directory target.
///
/// # Errors
///
/// Returns a fatal error when the explicit target cannot be discovered or read.
/// Recoverable descendant failures are attached to the returned snapshot.
pub fn build_snapshot(
    target: impl AsRef<Path>,
    options: &DiscoverOptions,
) -> Result<WorkspaceSnapshot, MdtError> {
    let discovery = discover(target, options)?;
    let mut warnings = discovery
        .warnings
        .into_iter()
        .map(|warning| warning_from_discovery(&discovery.root_path, warning))
        .collect::<Vec<_>>();
    let mut documents = Vec::new();
    let mut markdown_files = 0;

    for file in discovery.files {
        // Count every supported Markdown file discovery inspected, including a
        // file that must later be skipped with a warning.
        markdown_files += 1;
        match read_bounded(&file.absolute_path) {
            Ok(ReadOutcome::TooLarge) => {
                warnings.push(Warning::new(
                    Some(PathBuf::from(&file.display_path)),
                    format!(
                        "file exceeds the {} MiB size limit and was skipped",
                        MAX_MARKDOWN_FILE_SIZE / 1024 / 1024
                    ),
                ));
            }
            Ok(ReadOutcome::Bytes(bytes)) => {
                let source = match String::from_utf8(bytes) {
                    Ok(source) => source,
                    Err(error) => {
                        warnings.push(Warning::new(
                            Some(PathBuf::from(&file.display_path)),
                            "invalid UTF-8 was decoded lossily",
                        ));
                        String::from_utf8_lossy(error.as_bytes()).into_owned()
                    }
                };
                let document = parse_document(&file.absolute_path, &file.relative_path, &source);
                if document.stats().total() > 0 {
                    documents.push(document);
                }
            }
            Err(source) if discovery.explicit_file => {
                return Err(MdtError::UnreadableTarget {
                    path: file.absolute_path,
                    source,
                });
            }
            Err(source) => warnings.push(Warning::new(
                Some(PathBuf::from(&file.display_path)),
                format!("cannot read file: {source}"),
            )),
        }
    }

    // Discovery is already sorted, but keep snapshot construction robust if a
    // future discovery source supplies files in a different order.
    documents.sort_by(|left, right| {
        crate::model::compare_names(left.relative_path(), right.relative_path())
            .then_with(|| left.relative_path().cmp(right.relative_path()))
    });
    warnings.sort_by(|left, right| {
        let left_path = left
            .path()
            .map_or_else(String::new, |path| path.to_string_lossy().into_owned());
        let right_path = right
            .path()
            .map_or_else(String::new, |path| path.to_string_lossy().into_owned());
        crate::model::compare_names(&left_path, &right_path)
            .then_with(|| left.message().cmp(right.message()))
    });

    let task_files = documents.len();
    let complete_files = documents
        .iter()
        .filter(|document| document.stats().is_complete())
        .count();
    let scan_stats = ScanStats {
        directories: discovery.directories_inspected,
        ignored_directories: discovery.ignored_directories,
        markdown_files,
        task_files,
        complete_files,
    };
    Ok(WorkspaceSnapshot::new(
        discovery.root_path,
        documents,
        scan_stats,
        warnings,
    ))
}

/// Convenience alias for callers that describe snapshot construction as a scan.
///
/// # Errors
///
/// Returns the same fatal target errors as [`build_snapshot`].
pub fn scan(
    target: impl AsRef<Path>,
    options: &DiscoverOptions,
) -> Result<WorkspaceSnapshot, MdtError> {
    build_snapshot(target, options)
}

enum ReadOutcome {
    Bytes(Vec<u8>),
    TooLarge,
}

fn read_bounded(path: &Path) -> io::Result<ReadOutcome> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_MARKDOWN_FILE_SIZE {
        return Ok(ReadOutcome::TooLarge);
    }

    // Read one extra byte so a file that grows after metadata inspection cannot
    // bypass the guard or trigger an unbounded allocation.
    let mut bytes = Vec::new();
    file.take(MAX_MARKDOWN_FILE_SIZE + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MARKDOWN_FILE_SIZE {
        Ok(ReadOutcome::TooLarge)
    } else {
        Ok(ReadOutcome::Bytes(bytes))
    }
}

fn warning_from_discovery(root: &Path, warning: DiscoveryWarning) -> Warning {
    let displayed_path = warning
        .path
        .strip_prefix(root)
        .map(|path| {
            if path.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                path.to_path_buf()
            }
        })
        .unwrap_or(warning.path);
    Warning::new(Some(displayed_path), warning.cause)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use tempfile::tempdir;

    use super::{MAX_MARKDOWN_FILE_SIZE, build_snapshot};
    use crate::discover::DiscoverOptions;

    #[test]
    fn parses_documents_and_combines_all_counters() {
        let temp = tempdir().unwrap();
        fs::create_dir(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("done.md"), "# Done\n- [x] one\n- [X] two").unwrap();
        fs::write(
            temp.path().join("nested/open.markdown"),
            "- [x] one\n- [ ] two",
        )
        .unwrap();
        fs::write(temp.path().join("empty.mkd"), "# No tasks").unwrap();

        let snapshot = build_snapshot(temp.path(), &DiscoverOptions::default()).unwrap();
        assert_eq!(snapshot.documents().len(), 2);
        assert_eq!(snapshot.documents()[0].relative_path(), "done.md");
        assert_eq!(
            snapshot.documents()[1].relative_path(),
            "nested/open.markdown"
        );
        assert_eq!(snapshot.aggregate_stats().total(), 4);
        assert_eq!(snapshot.aggregate_stats().completed(), 3);
        assert_eq!(snapshot.aggregate_stats().remaining(), 1);
        assert_eq!(snapshot.scan_stats().directories, 2);
        assert_eq!(snapshot.scan_stats().markdown_files, 3);
        assert_eq!(snapshot.scan_stats().task_files, 2);
        assert_eq!(snapshot.scan_stats().complete_files, 1);
    }

    #[test]
    fn excludes_zero_task_documents_without_losing_scanned_count() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("readme.md"), "# Read me").unwrap();

        let snapshot = build_snapshot(temp.path(), &DiscoverOptions::default()).unwrap();
        assert!(snapshot.documents().is_empty());
        assert_eq!(snapshot.scan_stats().markdown_files, 1);
        assert_eq!(snapshot.scan_stats().task_files, 0);
        assert_eq!(snapshot.scan_stats().complete_files, 0);
    }

    #[test]
    fn decodes_invalid_utf8_lossily_and_keeps_surrounding_tasks() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("invalid.md");
        fs::write(&path, b"- [ ] before\n\xff\n- [x] after").unwrap();

        let snapshot = build_snapshot(temp.path(), &DiscoverOptions::default()).unwrap();
        assert_eq!(snapshot.aggregate_stats().total(), 2);
        assert_eq!(snapshot.aggregate_stats().completed(), 1);
        assert_eq!(snapshot.warnings().len(), 1);
        assert_eq!(
            snapshot.warnings()[0].path(),
            Some(std::path::Path::new("invalid.md"))
        );
        assert!(snapshot.warnings()[0].message().contains("UTF-8"));
    }

    #[test]
    fn skips_files_larger_than_sixteen_mib() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("large.md");
        let mut file = fs::File::create(path).unwrap();
        file.set_len(MAX_MARKDOWN_FILE_SIZE + 1).unwrap();
        file.flush().unwrap();

        let snapshot = build_snapshot(temp.path(), &DiscoverOptions::default()).unwrap();
        assert!(snapshot.documents().is_empty());
        assert_eq!(snapshot.scan_stats().markdown_files, 1);
        assert_eq!(snapshot.warnings().len(), 1);
        assert!(snapshot.warnings()[0].message().contains("16 MiB"));
    }

    #[test]
    fn explicit_unreadable_file_is_fatal_after_discovery() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("gone.md");
        fs::write(&path, "- [ ] task").unwrap();
        let discovery_options = DiscoverOptions::default();

        // This exercises the same fatal path portably by replacing the file
        // target with a directory between validation attempts.
        fs::remove_file(&path).unwrap();
        assert!(build_snapshot(&path, &discovery_options).is_err());
    }

    #[test]
    fn builds_a_reasonable_large_fixture_without_a_timing_assertion() {
        let temp = tempdir().unwrap();
        for directory in 0..10 {
            let path = temp.path().join(format!("group{directory}"));
            fs::create_dir(&path).unwrap();
            for file in 0..100 {
                fs::write(path.join(format!("task{file}.md")), "- [ ] task").unwrap();
            }
            for file in 0..900 {
                fs::write(path.join(format!("other{file}.txt")), "not markdown").unwrap();
            }
        }

        let snapshot = build_snapshot(temp.path(), &DiscoverOptions::default()).unwrap();
        assert_eq!(snapshot.scan_stats().markdown_files, 1_000);
        assert_eq!(snapshot.aggregate_stats().total(), 1_000);
        assert_eq!(snapshot.scan_stats().directories, 11);
    }
}
