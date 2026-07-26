# Group 02 — Task model and Markdown parser

Depends on: Group 01

Outcome: Markdown text becomes a stable, recursively aggregated task tree with the
same useful parsing semantics as Terminay.

## Model

- [ ] Define workspace, document, section, task, stats, scan-stats, and warning
      types in the library.
- [ ] Make task trees immutable after construction.
- [ ] Implement recursive stats aggregation, completion, remaining, and rounded
      percent helpers.
- [ ] Implement stable document, section, and task keys with duplicate occurrence
      disambiguation.
- [ ] Retain source line number, indentation depth, and original source order.

## Parser

- [ ] Implement ATX heading recognition for levels 1–6.
- [ ] Implement bullet and ordered checkbox markers with `[ ]`, `[x]`, and `[X]`.
- [ ] Implement task nesting from spaces/tabs and reset it at headings.
- [ ] Build the heading stack correctly when levels repeat or skip.
- [ ] Ignore backtick/tilde fenced-code contents and handle matching fence close.
- [ ] Preserve trimmed labels, including inline Markdown characters and Unicode.
- [ ] Support LF and CRLF without leaking `\r` into labels.
- [ ] Return a valid zero-task document rather than treating it as an error.

## View-independent ordering

- [ ] Implement numeric-aware case-insensitive name comparison.
- [ ] Implement progress comparison: percent descending, remaining ascending,
      name ascending.
- [ ] Keep sorting outside the parsed model.

## Verification

- [ ] Port representative examples from Terminay's parser and E2E tests.
- [ ] Add table-driven fixtures for every syntax and edge case in the feature spec.
- [ ] Add property tests that stats never exceed totals and remaining is exact.
- [ ] Add regression tests for repeated headings/labels and stable keys across
      unrelated line insertions.
