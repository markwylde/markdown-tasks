# Group 02 — Task model and Markdown parser

Depends on: Group 01

Outcome: Markdown text becomes a stable, recursively aggregated task tree with the
same useful parsing semantics as Terminay.

## Model

- [x] Define workspace, document, section, task, stats, scan-stats, and warning
      types in the library.
- [x] Make task trees immutable after construction.
- [x] Implement recursive stats aggregation, completion, remaining, and rounded
      percent helpers.
- [x] Implement stable document, section, and task keys with duplicate occurrence
      disambiguation.
- [x] Retain source line number, indentation depth, and original source order.

## Parser

- [x] Implement ATX heading recognition for levels 1–6.
- [x] Implement bullet and ordered checkbox markers with `[ ]`, `[x]`, and `[X]`.
- [x] Implement task nesting from spaces/tabs and reset it at headings.
- [x] Build the heading stack correctly when levels repeat or skip.
- [x] Ignore backtick/tilde fenced-code contents and handle matching fence close.
- [x] Preserve trimmed labels, including inline Markdown characters and Unicode.
- [x] Support LF and CRLF without leaking `\r` into labels.
- [x] Return a valid zero-task document rather than treating it as an error.

## View-independent ordering

- [x] Implement numeric-aware case-insensitive name comparison.
- [x] Implement progress comparison: percent descending, remaining ascending,
      name ascending.
- [x] Keep sorting outside the parsed model.

## Verification

- [x] Port representative examples from Terminay's parser and E2E tests.
- [x] Add table-driven fixtures for every syntax and edge case in the feature spec.
- [x] Add property tests that stats never exceed totals and remaining is exact.
- [x] Add regression tests for repeated headings/labels and stable keys across
      unrelated line insertions.
