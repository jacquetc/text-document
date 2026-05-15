# Memory profile: Phase 1 work-in-progress vs `pre-rope` baseline

Captured at `feature/rope-backend` HEAD (Phase 1.13b complete), comparing
to the `baseline-pre-rope` tag.

**Phase 1 acceptance criteria are NOT yet met** because `inline_elements`
has not been deleted. The auto-sync hook from Phase 1.5–1.12 maintains
`format_runs`/`block_images` in parallel with `inline_elements`, so the
current state has BOTH representations in memory — strictly more than
the baseline.

The Phase 1 wins only materialize after Phase 1.14 (deletion of the
`inline_elements` table + junction).

| Scenario | Pre-rope | Phase 1 WIP | Delta |
|---|---|---|---|
| A. 3 KB markdown (undo cleared) | 451.51 KiB | ~450 KiB | flat |
| B. + 10 char inserts (undo kept) | 481.30 KiB | ~474 KiB | flat |
| C. select-all+paste (undo kept) | 1.08 MiB | 1.15 MiB | **+7%** |
| D. 2× content directly | 698.34 KiB | 849.07 KiB | **+22%** |
| E. empty doc floor | 77.06 KiB | 80.11 KiB | +4% |
| F. 1 MB plain text | 1.98 MiB | 1.99 MiB | flat |
| G. 100×1 KB blocks | 1008.95 KiB | 1.07 MiB | +9% |
| H. 3 KB with bold every 5 chars | 1.55 MiB | 1.81 MiB | **+17%** |
| I. 100-cell table | 1.44 MiB | 1.55 MiB | +8% |
| J. 100 KB doc + 1000 inserts (undo kept) | 192.80 MiB | 192.80 MiB | **flat** |

The +22% on D is expected: doc has 2× content → 2× format_run entries
added on top of 2× inline_elements. The +17% on H is the same dynamic
in a format-run-heavy regime (600 runs).

J is unchanged because the bottleneck is in the simple-insert undo
path's hand-rolled inverse, which still clones the full InlineElement.
That cleanup belongs to Phase 1.14 (when the hand-rolled inverse can
switch to byte-range deltas, since InlineElement won't exist).

## What this means for Phase 1 status

- Phase 0 (baseline): complete ✓
- Phase 1.1–1.12 (foundations + auto-sync): complete ✓
- Phase 1.13a (text_block.fragments): complete ✓
- Phase 1.13b (cursor.char_format): complete ✓
- Phase 1.13c (use case readers): complete ✓ — `store()` plumbed on
  both UoW traits + 7 readers migrated (extract_fragment_uc,
  get_text_at_position_uc, get_document_stats_uc, export_html_uc,
  export_markdown_uc, export_docx_uc, export_latex_uc). The
  `replace_text_uc` reader is part of a writer path and deferred
  to 1.13e. Memory unchanged: dual-write still active.
- Phase 1.13d (DocumentFragment internal): N/A — FragmentElement
  schema is preserved verbatim; the conversion `from_entity` still
  consumes an `InlineElement`, but now the InlineElement is
  synthesized via `synthesize_block_inline_elements`. After Phase
  1.14 the `from_entity`/`to_entity` methods are rewritten to map
  directly to/from `FormatRun`+`ImageAnchor`.
- Phase 1.13e (writer migration): IN PROGRESS — 3 of 13 use cases
  migrated:
  * `insert_image_uc` — writes `block_images` directly, drops 4
    InlineElement decorators.
  * `set_text_format_uc` — replaces inline_element fmt_* updates with
    `format_runs` splice (per-run merge of dto Optional fields).
  * `merge_text_format_uc` — same splice shape, drops 8 decorators
    total across the two formatting use cases.
  Test surface migrated alongside: editing_extended_tests +
  complex_format_tests + formatting_tests + text_format_tests +
  merge_text_format_tests now read post-mutation state via
  `test_harness::synth_block_elements` (which materialises the view
  from format_runs / block_images).

  The remaining 10 writers share `block.plain_text` as their content
  source-of-truth and must be migrated as a coupled group:
  `insert_text_uc`, `delete_text_uc`, `insert_block_uc`,
  `insert_formatted_text_uc`, `insert_fragment_uc`,
  `insert_html_at_position_uc`, `insert_markdown_at_position_uc`,
  `import_{plain_text,html,markdown}_uc`, `replace_text_uc`.

  Why coupled: each one mutates `block.plain_text` and
  `block.text_length`. If only insert_text migrates (stops touching
  inline_elements), a later delete_text rebuilds plain_text from the
  now-stale inline_elements and silently drops the inserted chars.
  Attempted in this session, reverted after `random_edit_sequence_
  preserves_invariants` proptest caught the mismatch on
  `[InsertText("aa"), DeletePrev]`. They migrate together or not at
  all.
- Phase 1.14 (deletion): NOT STARTED — depends on 1.13e. Memory
  wins materialize here.
- Phase 1.17 (bench compare): N/A until 1.14 done.

The current state is a stable WIP checkpoint that Phase 2 (rope swap)
could pick up from directly. Phase 2 naturally subsumes 1.14 because
the rope replaces inline_elements entirely.
