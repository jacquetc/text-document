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
- Phase 1.13e (writer migration): COMPLETE ✓ — 13 of 13 use cases
  migrated:
  * `insert_image_uc` — writes `block_images` directly.
  * `set_text_format_uc` — replaces inline_element fmt_* updates with
    `format_runs` splice (per-run merge of dto Optional fields).
  * `merge_text_format_uc` — same splice shape; 8 decorators removed
    across the two formatting use cases.
  * `insert_text_uc` + `delete_text_uc` + `insert_block_uc` — the
    text-content trio. They share `block.plain_text` as the content
    source-of-truth, so they were migrated atomically together with
    a `rebuild_block_inline_elements` reverse-sync helper that
    materialises a consistent inline_elements view from the new
    (plain_text + format_runs + block_images) representation.
  * `import_plain_text_uc` — populates `block.plain_text` directly
    and reverse-syncs an Empty / Text inline_element per block so
    legacy writers reading inline_elements still see the right
    content.
  * `insert_formatted_text_uc` — same shape as insert_text but
    splices a single FormatRun carrying the dto's character format
    over the inserted byte range (overrides any inherited format
    from the surrounding run).
  * `replace_text_uc` — byte-range delete-then-insert per match,
    inheriting surrounding format on the replacement bytes.
  * `import_html_uc` + `import_markdown_uc` — populate `block.
    plain_text` and `format_runs` from `format_runs_from_spans`
    (new helper in `parser_tools::content_parser`); reverse-sync
    per block. `block_images` stays empty (parsers don't surface
    inline images today).
  * `insert_markdown_at_position_uc` + `insert_html_at_position_uc`
    — three insertion shapes each (inline-merge, multi-block, single
    block-level). Inline-merge uses `shift_runs_for_insert` +
    `splice_range` over the inserted byte range. Multi-block and
    single-block-level use `split_runs_at` + `split_images_at` on
    the current block at the byte boundary, with head/tail states
    rebuilt from the split halves plus the parsed block's runs.
    Both files inline the macro `impl_content_insert!` that the
    legacy code shared, since the HTML and Markdown variants
    diverged enough that the shared shape was no longer pulling
    its weight. HTML additionally carries `overwrite_head` and
    `skip_tail` semantics that Markdown doesn't.
  * `insert_fragment_uc` — the copy/paste pivot (~92 KB pre-
    migration). Same three shapes as the insert-at-position
    writers, plus table-fragment paths (`insert_table_fragment`
    and `try_replace_table_cells`) and a mixed-fragment path
    (`insert_mixed_fragment`) that interleaves blocks with tables.
    `frag_block_state()` reuses `format_runs_from_inline_elements`
    via `FragmentElement::to_entity()`, so image-bearing fragments
    naturally carry block_images through paste.

  Cell-frame seed refactor: `create_cell_frame` (used by the 4
  unmigrated table-structural writers plus `insert_fragment_uc`)
  no longer calls `cfc_create_inline_element` to plant an Empty
  inline_element on each new cell block. It now calls
  `rebuild_block_inline_elements` to synthesize the same Empty
  fallback from the (empty) format_runs / block_images. The
  CellFrameCreator trait swaps `cfc_create_inline_element` for
  `cfc_store`. As a result, `insert_table_uc`, `insert_table_
  column_uc`, `insert_table_row_uc`, and `split_table_cell_uc`
  also dropped their `InlineElement Create` decorators — they no
  longer touch inline_elements at all even though their bodies
  still go through the legacy path otherwise.

  Compatibility bridge: the auto-sync hook (inline_elements →
  format_runs) and reverse-sync (format_runs → inline_elements)
  keep both representations consistent under any mix of legacy
  and migrated paths. Three subtle bugs surfaced and were fixed
  during this work:
  * `shift_runs_for_delete` now coalesces post-shift (adjacent
    identical default-format runs were appearing after delete).
  * `rebuild_block_inline_elements` uses the store's snake_case
    counter key `"inline_element"` (mismatched CamelCase caused
    ID collisions with legacy creates).
  * `shift_runs_for_insert`'s extension condition uses
    `byte_end >= byte_offset` (not strict `>`) so that a run
    ending exactly at the insertion point extends to cover the
    inserted bytes — matches Qt's format-inheritance convention.

  Test-shape adjustment: a fragment round-trip test counted
  inline_element nodes carrying "bold". Under the new model
  adjacent same-format runs coalesce into one run / one inline
  element, so the test now counts "bold text" substring
  occurrences inside bold-formatted elements — same intent,
  representation-independent assertion.

- Phase 1.14 (deletion): UNBLOCKED — every writer now goes through
  (plain_text, format_runs, block_images); inline_elements is fully
  synthetic via reverse-sync. Memory wins materialize here.
- Phase 1.17 (bench compare): N/A until 1.14 done.

The current state is a stable WIP checkpoint that Phase 2 (rope swap)
could pick up from directly. Phase 2 naturally subsumes 1.14 because
the rope replaces inline_elements entirely.

## Known pre-existing bug (out of scope for 1.13e)

During the migration runs proptest surfaced a long-standing cursor /
insert_block overflow that reproduces on the pre-rope baseline
(`baseline-pre-rope` tag) too:

  seed = "", ops = [InsertBlock, MovePrev(1), InsertText("a "),
                    SelectBackward(2), InsertText("  "),
                    InsertText(""), Undo, InsertBlock]

The terminal `InsertBlock` triggers a `new_pos - edit_pos` underflow
in `cursor::finish_edit_ext` at `crates/public_api/src/cursor.rs:108`.
The bug predates the rope-backend branch — it's reproducible on
`e2c4de6` (pre-rope baseline) and `ddea418` (pre-1.13e baseline).
The captured seed was deliberately *not* persisted to
`fuzz_robustness_tests.proptest-regressions` so it doesn't block
other migration work; track separately.
