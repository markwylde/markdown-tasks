//! Markdown task parsing.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::model::{Document, Section, Task};

#[derive(Debug)]
struct SectionBuilder {
    key: String,
    title: Option<String>,
    level: u8,
    tasks: Vec<Task>,
    children: Vec<usize>,
    occurrences: HashMap<String, usize>,
    task_occurrences: HashMap<String, usize>,
}

impl SectionBuilder {
    fn root(key: String) -> Self {
        Self {
            key,
            title: None,
            level: 0,
            tasks: Vec::new(),
            children: Vec::new(),
            occurrences: HashMap::new(),
            task_occurrences: HashMap::new(),
        }
    }
}

/// Parse Markdown text into a document.
#[must_use]
pub fn parse_document(
    absolute_path: impl Into<PathBuf>,
    relative_path: impl AsRef<Path>,
    source: &str,
) -> Document {
    let absolute_path = absolute_path.into();
    let relative_path = normalize_relative_path(relative_path.as_ref());
    let document_key = format!("doc:{}", encoded_component(&relative_path));
    let root_key = format!("sec:{document_key}:root");
    let mut sections = vec![SectionBuilder::root(root_key)];
    let mut heading_stack: Vec<(u8, usize)> = Vec::new();
    let mut current = 0;
    let mut indent_stack: Vec<usize> = Vec::new();
    let mut fence: Option<(char, usize)> = None;

    for (line_index, raw_line) in source.lines().enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if let Some((fence_char, fence_len)) = fence {
            indent_stack.clear();
            if is_fence_close(line, fence_char, fence_len) {
                fence = None;
            }
            continue;
        }
        if let Some(opening) = fence_open(line) {
            indent_stack.clear();
            fence = Some(opening);
            continue;
        }

        if let Some((level, title)) = parse_heading(line) {
            while heading_stack
                .last()
                .is_some_and(|(stack_level, _)| *stack_level >= level)
            {
                heading_stack.pop();
            }
            let parent = heading_stack.last().map_or(0, |(_, index)| *index);
            let normalized = normalize_identity_text(&title);
            let occurrence = {
                let value = sections[parent]
                    .occurrences
                    .entry(normalized.clone())
                    .and_modify(|value| *value += 1)
                    .or_insert(0);
                *value
            };
            let key = format!(
                "{}|h{}:{}#{}",
                sections[parent].key,
                level,
                encoded_component(&normalized),
                occurrence
            );
            let index = sections.len();
            sections.push(SectionBuilder {
                key,
                title: Some(title),
                level,
                tasks: Vec::new(),
                children: Vec::new(),
                occurrences: HashMap::new(),
                task_occurrences: HashMap::new(),
            });
            sections[parent].children.push(index);
            heading_stack.push((level, index));
            current = index;
            indent_stack.clear();
            continue;
        }

        if let Some(parsed) = parse_task(line) {
            let depth = indentation_depth(&mut indent_stack, parsed.indent);
            let normalized = normalize_identity_text(&parsed.label);
            let occurrence = {
                let value = sections[current]
                    .task_occurrences
                    .entry(normalized.clone())
                    .and_modify(|value| *value += 1)
                    .or_insert(0);
                *value
            };
            let key = format!(
                "task:{}|{}:{}#{}",
                document_key,
                sections[current].key,
                encoded_component(&normalized),
                occurrence
            );
            sections[current].tasks.push(Task::new(
                key,
                parsed.label,
                parsed.checked,
                depth,
                line_index + 1,
            ));
        } else {
            // Indentation only relates tasks within one uninterrupted run of
            // task lines. Prose, blank lines, and other list items start a new
            // task sequence, so the next task becomes a root at its location.
            indent_stack.clear();
        }
    }

    let root = finish_section(0, &sections);
    Document::new(document_key, absolute_path, relative_path, root)
}

/// Alias useful to callers that think in terms of input format rather than
/// document construction.
#[must_use]
pub fn parse_markdown(
    absolute_path: impl Into<PathBuf>,
    relative_path: impl AsRef<Path>,
    source: &str,
) -> Document {
    parse_document(absolute_path, relative_path, source)
}

fn finish_section(index: usize, builders: &[SectionBuilder]) -> Section {
    let builder = &builders[index];
    let children = builder
        .children
        .iter()
        .map(|child| finish_section(*child, builders))
        .collect();
    Section::new(
        builder.key.clone(),
        builder.title.clone(),
        builder.level,
        builder.tasks.clone(),
        children,
    )
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedTask {
    indent: usize,
    checked: bool,
    label: String,
}

fn parse_task(line: &str) -> Option<ParsedTask> {
    let indent_bytes = line
        .char_indices()
        .take_while(|(_, character)| matches!(character, ' ' | '\t'))
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    let indent = line[..indent_bytes]
        .chars()
        .map(|character| if character == '\t' { 2 } else { 1 })
        .sum();
    let rest = &line[indent_bytes..];

    let after_marker = if let Some(rest) = rest.strip_prefix(['-', '*', '+']) {
        strip_required_whitespace(rest)?
    } else {
        let digit_count = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digit_count == 0 {
            return None;
        }
        let rest = rest.get(digit_count..)?;
        let rest = rest.strip_prefix(['.', ')'])?;
        strip_required_whitespace(rest)?
    };

    let checked = match after_marker.get(..3)? {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let label = after_marker.get(3..)?;
    if !label.is_empty() && !label.starts_with(char::is_whitespace) {
        return None;
    }
    Some(ParsedTask {
        indent,
        checked,
        label: label.trim().to_owned(),
    })
}

fn strip_required_whitespace(value: &str) -> Option<&str> {
    let offset = value
        .char_indices()
        .take_while(|(_, character)| character.is_whitespace())
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    value.get(offset..)
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = trimmed.get(hashes..)?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let title = rest.trim();
    let title = strip_closing_hashes(title);
    Some((u8::try_from(hashes).ok()?, title.to_owned()))
}

fn strip_closing_hashes(title: &str) -> &str {
    let without_hashes = title.trim_end_matches('#');
    if without_hashes.len() < title.len()
        && without_hashes
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        without_hashes.trim_end()
    } else {
        title
    }
}

fn fence_open(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let character = trimmed.chars().next()?;
    if !matches!(character, '`' | '~') {
        return None;
    }
    let length = trimmed
        .chars()
        .take_while(|value| *value == character)
        .count();
    (length >= 3).then_some((character, length))
}

fn is_fence_close(line: &str, character: char, opening_length: usize) -> bool {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let length = trimmed
        .chars()
        .take_while(|value| *value == character)
        .count();
    length >= opening_length
        && trimmed
            .get(length..)
            .is_some_and(|suffix| suffix.trim().is_empty())
}

fn indentation_depth(stack: &mut Vec<usize>, width: usize) -> usize {
    if stack.is_empty() {
        stack.push(width);
        return 0;
    }
    while stack.last().is_some_and(|indent| *indent > width) {
        stack.pop();
    }
    if stack.last().is_none_or(|indent| *indent < width) {
        stack.push(width);
    }
    stack.len() - 1
}

fn normalize_relative_path(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir | Component::RootDir => {}
            Component::ParentDir => {
                if parts.last().is_some_and(|part| part != "..") {
                    parts.pop();
                } else {
                    parts.push("..".to_owned());
                }
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_string_lossy().into_owned());
            }
        }
    }
    parts.join("/")
}

fn normalize_identity_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn encoded_component(value: &str) -> String {
    format!("{}:{value}", value.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Document {
        parse_document("/workspace/tasks.md", "./notes/../tasks.md", source)
    }

    fn all_tasks(section: &Section) -> Vec<&Task> {
        let mut result: Vec<_> = section.tasks().iter().collect();
        for child in section.children() {
            result.extend(all_tasks(child));
        }
        result
    }

    #[test]
    fn accepts_every_marker_checkbox_case_and_line_ending() {
        let document =
            parse("- [ ] dash\r\n* [x] star\r\n+ [X] plus\r\n1. [ ] dot\r\n2) [x] paren\r\n");
        let tasks = all_tasks(document.root());
        assert_eq!(tasks.len(), 5);
        assert_eq!(
            tasks.iter().map(|task| task.checked()).collect::<Vec<_>>(),
            vec![false, true, true, false, true]
        );
        assert!(tasks.iter().all(|task| !task.label().contains('\r')));
        assert_eq!(tasks[4].line_number(), 5);
    }

    #[test]
    fn builds_heading_tree_when_levels_repeat_and_skip() {
        let document =
            parse("# A\n- [ ] a\n### Deep\n- [ ] deep\n## Middle\n- [x] middle\n# B\n- [ ] b");
        let root = document.root();
        assert_eq!(root.children().len(), 2);
        assert_eq!(root.children()[0].title(), Some("A"));
        assert_eq!(root.children()[0].children().len(), 2);
        assert_eq!(root.children()[0].children()[0].level(), 3);
        assert_eq!(root.children()[0].children()[1].level(), 2);
        assert_eq!(document.stats().total(), 4);
        assert_eq!(document.stats().completed(), 1);
    }

    #[test]
    fn root_tasks_and_heading_tasks_stay_in_source_groups() {
        let document = parse("- [ ] root\n# Heading\n- [x] child");
        assert_eq!(document.root().tasks()[0].label(), "root");
        assert_eq!(document.root().children()[0].tasks()[0].label(), "child");
    }

    #[test]
    fn derives_depth_from_increasing_indentation_and_resets_at_heading() {
        let document =
            parse("- [ ] zero\n  - [ ] one\n\t\t- [ ] two\n - [ ] one again\n# H\n    - [ ] reset");
        let tasks = all_tasks(document.root());
        assert_eq!(
            tasks.iter().map(|task| task.depth()).collect::<Vec<_>>(),
            vec![0, 1, 2, 1, 0]
        );
    }

    #[test]
    fn preserves_nesting_only_across_consecutive_task_lines() {
        let document = parse("- [ ] parent\n  - [ ] child\n    - [ ] grandchild\n- [ ] sibling");
        let tasks = all_tasks(document.root());
        assert_eq!(
            tasks.iter().map(|task| task.depth()).collect::<Vec<_>>(),
            vec![0, 1, 2, 0]
        );
    }

    #[test]
    fn resets_nesting_across_blank_prose_and_non_task_list_lines() {
        let document = parse(
            "- [ ] first\n  - [ ] nested\n\n    - [ ] after blank\n\
             prose between tasks\n      - [ ] after prose\n\
             - ordinary list item\n        - [ ] after non-task list",
        );
        let tasks = all_tasks(document.root());
        assert_eq!(
            tasks.iter().map(|task| task.depth()).collect::<Vec<_>>(),
            vec![0, 1, 0, 0, 0]
        );
    }

    #[test]
    fn resets_nesting_across_fenced_code_blocks() {
        let document =
            parse("- [ ] before\n  - [ ] nested\n```\nignored\n```\n    - [ ] after fence");
        let tasks = all_tasks(document.root());
        assert_eq!(
            tasks.iter().map(|task| task.depth()).collect::<Vec<_>>(),
            vec![0, 1, 0]
        );
    }

    #[test]
    fn ignores_both_kinds_of_indented_fences_and_non_list_prose() {
        let document = parse(
            "  ```rust\n- [ ] hidden\n  ```\n~~~\n1. [x] hidden too\n~~~~\n[ ] prose\n- [ ] visible",
        );
        let tasks = all_tasks(document.root());
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].label(), "visible");
    }

    #[test]
    fn preserves_unicode_inline_markdown_and_empty_labels() {
        let document = parse("- [ ] **café** 東京 🔥\n+ [X]    ");
        let tasks = all_tasks(document.root());
        assert_eq!(tasks[0].label(), "**café** 東京 🔥");
        assert_eq!(tasks[1].label(), "");
    }

    #[test]
    fn malformed_checkboxes_are_ignored() {
        let document =
            parse("- [] no\n- [y] no\n- [ x ] no\n- [ ]no-space\n1 [ ] no\n0: [ ] no\n- [x] yes");
        let tasks = all_tasks(document.root());
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].label(), "yes");
    }

    #[test]
    fn zero_task_document_is_valid() {
        let document = parse("# Notes\nNothing here");
        assert_eq!(document.stats().total(), 0);
        assert_eq!(document.root().children()[0].title(), Some("Notes"));
    }

    #[test]
    fn duplicate_keys_are_disambiguated_and_stable_across_unrelated_insertions() {
        let before = parse("# Same\n- [ ] repeat\n- [ ] repeat\n# Same\n- [ ] repeat");
        let after =
            parse("unrelated prose\n# Same\n- [ ] repeat\n- [ ] repeat\n# Same\n- [ ] repeat");
        let before_sections = before.root().children();
        let after_sections = after.root().children();
        assert_ne!(before_sections[0].key(), before_sections[1].key());
        assert_eq!(before_sections[0].key(), after_sections[0].key());
        assert_eq!(before_sections[1].key(), after_sections[1].key());
        assert_ne!(
            before_sections[0].tasks()[0].key(),
            before_sections[0].tasks()[1].key()
        );
        assert_eq!(
            before_sections[0].tasks()[1].key(),
            after_sections[0].tasks()[1].key()
        );
    }

    #[test]
    fn normalized_relative_path_is_the_document_identity() {
        let document = parse("- [ ] task");
        assert_eq!(document.relative_path(), "tasks.md");
        assert!(document.key().contains("tasks.md"));
    }
}
