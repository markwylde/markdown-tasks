//! Filesystem target validation and deterministic Markdown discovery.

use std::{
    cmp::Ordering,
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
};

use crate::error::MdtError;

/// Markdown extensions accepted for explicit files and recursive discovery.
pub const SUPPORTED_EXTENSIONS: [&str; 4] = ["md", "markdown", "mdown", "mkd"];

/// Directory names ignored unless default ignores are disabled.
pub const DEFAULT_IGNORED_DIRECTORIES: [&str; 17] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "bower_components",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".cache",
    "coverage",
    "target",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
];

/// User-controlled traversal settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoverOptions {
    /// Directory names or root-relative directory paths to skip.
    pub ignore: Vec<String>,
    /// Disable the built-in ignored-directory list.
    pub no_default_ignore: bool,
}

/// One supported Markdown file found beneath the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub absolute_path: PathBuf,
    /// Native relative path used for filesystem operations.
    pub relative_path: PathBuf,
    /// Stable display/key form, whose separators are always `/`.
    pub display_path: String,
}

/// A recoverable traversal problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryWarning {
    pub path: PathBuf,
    pub cause: String,
}

impl DiscoveryWarning {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, cause: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            cause: cause.into(),
        }
    }
}

/// Deterministic output from validating and traversing one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub root_path: PathBuf,
    pub files: Vec<DiscoveredFile>,
    pub directories_inspected: usize,
    pub ignored_directories: usize,
    pub warnings: Vec<DiscoveryWarning>,
    pub explicit_file: bool,
}

/// Validate only the explicit target shape without traversing a directory.
///
/// This lets the TUI enter promptly and perform its initial recursive scan on
/// the background worker while still rejecting bad input before terminal setup.
///
/// # Errors
///
/// Returns a fatal target error for a missing, unreadable, unsupported, or
/// non-regular explicit target.
pub fn validate_target(target: impl AsRef<Path>) -> Result<(), MdtError> {
    let requested = target.as_ref();
    let link_metadata = fs::symlink_metadata(requested).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            MdtError::TargetMissing(requested.to_path_buf())
        } else {
            MdtError::UnreadableTarget {
                path: requested.to_path_buf(),
                source,
            }
        }
    })?;
    let metadata = fs::metadata(requested).map_err(|source| MdtError::UnreadableTarget {
        path: requested.to_path_buf(),
        source,
    })?;
    if metadata.is_file() {
        return is_supported_markdown(requested)
            .then_some(())
            .ok_or_else(|| MdtError::UnsupportedFile(requested.to_path_buf()));
    }
    if metadata.is_dir() && !link_metadata.file_type().is_symlink() {
        return Ok(());
    }
    Err(MdtError::UnreadableTarget {
        path: requested.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "target is neither a regular file nor a directory",
        ),
    })
}

/// Validate `target` and discover supported Markdown files beneath it.
///
/// # Errors
///
/// Returns a fatal target error when the explicit target is missing, unreadable,
/// unsupported, or not a regular file/directory. Descendant failures are warnings.
pub fn discover(
    target: impl AsRef<Path>,
    options: &DiscoverOptions,
) -> Result<Discovery, MdtError> {
    let requested = target.as_ref();
    let link_metadata = fs::symlink_metadata(requested).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            MdtError::TargetMissing(requested.to_path_buf())
        } else {
            MdtError::UnreadableTarget {
                path: requested.to_path_buf(),
                source,
            }
        }
    })?;
    let metadata = fs::metadata(requested).map_err(|source| MdtError::UnreadableTarget {
        path: requested.to_path_buf(),
        source,
    })?;
    let absolute = absolute_path(requested).map_err(|source| MdtError::UnreadableTarget {
        path: requested.to_path_buf(),
        source,
    })?;

    if metadata.is_file() {
        if !is_supported_markdown(&absolute) {
            return Err(MdtError::UnsupportedFile(requested.to_path_buf()));
        }
        let relative_path = absolute
            .file_name()
            .map_or_else(|| absolute.clone(), PathBuf::from);
        return Ok(Discovery {
            root_path: absolute.clone(),
            files: vec![DiscoveredFile {
                display_path: normalized_relative_path(&relative_path),
                relative_path,
                absolute_path: absolute,
            }],
            directories_inspected: 0,
            ignored_directories: 0,
            warnings: Vec::new(),
            explicit_file: true,
        });
    }

    // Following an explicitly targeted directory symlink would violate the
    // directory-symlink policy even though `metadata` reports a directory.
    if metadata.is_dir() && !link_metadata.file_type().is_symlink() {
        let root_path =
            fs::canonicalize(requested).map_err(|source| MdtError::UnreadableTarget {
                path: requested.to_path_buf(),
                source,
            })?;
        let matcher = IgnoreMatcher::new(options);
        let mut discovery = Discovery {
            root_path: root_path.clone(),
            files: Vec::new(),
            directories_inspected: 0,
            ignored_directories: 0,
            warnings: Vec::new(),
            explicit_file: false,
        };
        scan_directory(&root_path, Path::new(""), true, &matcher, &mut discovery)?;
        discovery.files.sort_by(|left, right| {
            natural_compare(&left.display_path, &right.display_path)
                .then_with(|| left.display_path.cmp(&right.display_path))
        });
        discovery.warnings.sort_by(|left, right| {
            natural_compare(&left.path.to_string_lossy(), &right.path.to_string_lossy())
                .then_with(|| left.cause.cmp(&right.cause))
        });
        return Ok(discovery);
    }

    Err(MdtError::UnreadableTarget {
        path: requested.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "target is neither a regular file nor a directory",
        ),
    })
}

/// Return whether `path` has a supported extension, case-insensitively.
#[must_use]
pub fn is_supported_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

/// Convert a native relative path into its platform-independent display form.
#[must_use]
pub fn normalized_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            Component::ParentDir => Some("..".into()),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn scan_directory(
    directory: &Path,
    relative: &Path,
    is_root: bool,
    matcher: &IgnoreMatcher,
    discovery: &mut Discovery,
) -> Result<(), MdtError> {
    discovery.directories_inspected += 1;
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if is_root => {
            return Err(MdtError::UnreadableTarget {
                path: directory.to_path_buf(),
                source,
            });
        }
        Err(source) => {
            discovery.warnings.push(DiscoveryWarning::new(
                directory,
                format!("cannot enumerate directory: {source}"),
            ));
            return Ok(());
        }
    };

    let mut children = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => children.push(entry.path()),
            Err(source) => discovery.warnings.push(DiscoveryWarning::new(
                directory,
                format!("cannot inspect directory entry: {source}"),
            )),
        }
    }
    children.sort_by(|left, right| compare_paths(left, right));

    for path in children {
        let Some(name) = path.file_name() else {
            continue;
        };
        let child_relative = relative.join(name);
        let file_type = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata.file_type(),
            Err(source) => {
                discovery.warnings.push(DiscoveryWarning::new(
                    &path,
                    format!("cannot inspect path: {source}"),
                ));
                continue;
            }
        };

        // Recursive traversal never follows symlinks, whether their targets are
        // files or directories.
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if matcher.is_ignored(name, &child_relative) {
                discovery.ignored_directories += 1;
                continue;
            }
            scan_directory(&path, &child_relative, false, matcher, discovery)?;
        } else if file_type.is_file() && is_supported_markdown(&path) {
            discovery.files.push(DiscoveredFile {
                absolute_path: path,
                display_path: normalized_relative_path(&child_relative),
                relative_path: child_relative,
            });
        }
    }
    Ok(())
}

fn compare_paths(left: &Path, right: &Path) -> Ordering {
    let left = left
        .file_name()
        .unwrap_or(left.as_os_str())
        .to_string_lossy();
    let right = right
        .file_name()
        .unwrap_or(right.as_os_str())
        .to_string_lossy();
    natural_compare(&left, &right).then_with(|| left.cmp(&right))
}

fn natural_compare(left: &str, right: &str) -> Ordering {
    natord::compare_ignore_case(left, right)
}

#[derive(Debug)]
struct IgnoreMatcher {
    names: Vec<String>,
    paths: Vec<Vec<String>>,
}

impl IgnoreMatcher {
    fn new(options: &DiscoverOptions) -> Self {
        let mut names = if options.no_default_ignore {
            Vec::new()
        } else {
            DEFAULT_IGNORED_DIRECTORIES
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        let mut paths = Vec::new();
        for pattern in &options.ignore {
            let components = pattern_components(pattern);
            if components.is_empty() {
                continue;
            }
            if components.len() == 1 && !pattern.contains(['/', '\\']) {
                names.push(components[0].clone());
            } else {
                paths.push(components);
            }
        }
        Self { names, paths }
    }

    fn is_ignored(&self, name: &OsStr, relative: &Path) -> bool {
        let name = name.to_string_lossy();
        if self.names.iter().any(|ignored| ignored == &name) {
            return true;
        }
        let components = relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>();
        self.paths.iter().any(|ignored| ignored == &components)
    }
}

fn pattern_components(pattern: &str) -> Vec<String> {
    pattern
        .split(['/', '\\'])
        .filter(|component| !component.is_empty() && *component != ".")
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::{
        DEFAULT_IGNORED_DIRECTORIES, DiscoverOptions, discover, is_supported_markdown,
        normalized_relative_path,
    };

    #[test]
    fn recognizes_extensions_case_insensitively() {
        for path in ["a.md", "a.MARKDOWN", "a.MdOwN", "a.MKD"] {
            assert!(is_supported_markdown(Path::new(path)), "{path}");
        }
        for path in ["a", "a.txt", "a.md.bak"] {
            assert!(!is_supported_markdown(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn traverses_in_natural_deterministic_order() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("part2")).unwrap();
        fs::create_dir_all(temp.path().join("part10")).unwrap();
        fs::write(temp.path().join("part10/z.md"), "- [ ] z").unwrap();
        fs::write(temp.path().join("part2/B.MD"), "- [ ] b").unwrap();
        fs::write(temp.path().join("part2/a.md"), "- [ ] a").unwrap();
        fs::write(temp.path().join("ignored.txt"), "no").unwrap();

        let result = discover(temp.path(), &DiscoverOptions::default()).unwrap();
        let paths = result
            .files
            .iter()
            .map(|file| file.display_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["part2/a.md", "part2/B.MD", "part10/z.md"]);
        assert_eq!(result.directories_inspected, 3);
    }

    #[test]
    fn applies_default_name_and_explicit_path_ignores() {
        let temp = tempdir().unwrap();
        for directory in DEFAULT_IGNORED_DIRECTORIES {
            fs::create_dir_all(temp.path().join(directory)).unwrap();
            fs::write(temp.path().join(directory).join("task.md"), "- [ ] x").unwrap();
        }
        fs::create_dir_all(temp.path().join("one/generated")).unwrap();
        fs::create_dir_all(temp.path().join("two/generated")).unwrap();
        fs::write(temp.path().join("one/generated/a.md"), "- [ ] a").unwrap();
        fs::write(temp.path().join("two/generated/b.md"), "- [ ] b").unwrap();

        let result = discover(
            temp.path(),
            &DiscoverOptions {
                ignore: vec!["one/generated".into()],
                no_default_ignore: false,
            },
        )
        .unwrap();
        assert_eq!(
            result
                .files
                .iter()
                .map(|file| file.display_path.as_str())
                .collect::<Vec<_>>(),
            ["two/generated/b.md"]
        );
        assert_eq!(
            result.ignored_directories,
            DEFAULT_IGNORED_DIRECTORIES.len() + 1
        );
    }

    #[test]
    fn disabling_defaults_retains_explicit_ignores() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("target")).unwrap();
        fs::create_dir_all(temp.path().join("skip")).unwrap();
        fs::write(temp.path().join("target/a.md"), "- [ ] a").unwrap();
        fs::write(temp.path().join("skip/b.md"), "- [ ] b").unwrap();

        let result = discover(
            temp.path(),
            &DiscoverOptions {
                ignore: vec!["skip".into()],
                no_default_ignore: true,
            },
        )
        .unwrap();
        assert_eq!(result.files[0].display_path, "target/a.md");
        assert_eq!(result.ignored_directories, 1);
    }

    #[test]
    fn an_explicit_ignored_directory_is_still_the_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("target");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("task.md"), "- [ ] root").unwrap();

        let result = discover(&root, &DiscoverOptions::default()).unwrap();
        assert_eq!(result.files[0].display_path, "task.md");
        assert_eq!(result.directories_inspected, 1);
        assert_eq!(result.ignored_directories, 0);
    }

    #[test]
    fn explicit_supported_file_is_accepted() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("TASK.MD");
        fs::write(&file, "- [ ] task").unwrap();

        let result = discover(&file, &DiscoverOptions::default()).unwrap();
        assert!(result.explicit_file);
        assert_eq!(result.files[0].display_path, "TASK.MD");
        assert_eq!(result.directories_inspected, 0);
    }

    #[test]
    fn unsupported_explicit_file_is_rejected() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("task.txt");
        fs::write(&file, "- [ ] task").unwrap();

        assert!(discover(&file, &DiscoverOptions::default()).is_err());
    }

    #[test]
    fn display_paths_join_native_components_with_slashes() {
        assert_eq!(
            normalized_relative_path(Path::new("nested").join("task.md").as_path()),
            "nested/task.md"
        );
    }

    #[cfg(unix)]
    #[test]
    fn recursive_symlinks_are_skipped_but_an_explicit_file_symlink_is_allowed() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        fs::create_dir(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("nested/task.md"), "- [ ] task").unwrap();
        symlink(
            temp.path().join("nested"),
            temp.path().join("directory-link"),
        )
        .unwrap();
        symlink(
            temp.path().join("nested/task.md"),
            temp.path().join("file-link.md"),
        )
        .unwrap();

        let directory_result = discover(temp.path(), &DiscoverOptions::default()).unwrap();
        assert_eq!(
            directory_result
                .files
                .iter()
                .map(|file| file.display_path.as_str())
                .collect::<Vec<_>>(),
            ["nested/task.md"]
        );

        let file_result = discover(
            temp.path().join("file-link.md"),
            &DiscoverOptions::default(),
        )
        .unwrap();
        assert!(file_result.explicit_file);
        assert_eq!(file_result.files[0].display_path, "file-link.md");
    }
}
