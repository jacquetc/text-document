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
- Phase 1.13e (writer migration): IN PROGRESS — 9 of 13 use cases
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

  Compatibility bridge: the auto-sync hook (inline_elements →
  format_runs) stays enabled while writers migrate one-at-a-time;
  the new reverse-sync (format_runs → inline_elements) closes the
  loop the other way. Both representations stay consistent under
  any mix of migrated and unmigrated writer calls. Two subtle bugs
  surfaced and were fixed during this work:
  * `shift_runs_for_delete` now coalesces post-shift (adjacent
    identical default-format runs were appearing after delete).
  * `rebuild_block_inline_elements` uses the store's snake_case
    counter key `"inline_element"` (mismatched CamelCase caused
    ID collisions with legacy creates).

  The remaining 4 unmigrated writers continue to work via the
  legacy path + auto-sync (they read/write inline_elements; format_
  runs gets rebuilt from inline_elements automatically):
  `insert_fragment_uc`, `insert_html_at_position_uc`,
  `insert_markdown_at_position_uc`, `import_html_uc`,
  `import_markdown_uc`. They can now migrate one-at-a-time using
  the same dual-write pattern (reverse-sync after writing
  format_runs/block_images); no further atomic coupling remains.
- Phase 1.14 (deletion): NOT STARTED — depends on 1.13e. Memory
  wins materialize here.
- Phase 1.17 (bench compare): N/A until 1.14 done.

The current state is a stable WIP checkpoint that Phase 2 (rope swap)
could pick up from directly. Phase 2 naturally subsumes 1.14 because
the rope replaces inline_elements entirely.
