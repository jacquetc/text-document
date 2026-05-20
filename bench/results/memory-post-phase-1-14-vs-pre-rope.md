# Memory profile: post Phase 1.14 vs `pre-rope` baseline

Captured at `feature/rope-backend` HEAD (Phase 1.14a complete: the
inline_elements entity table, junction, controllers, repositories,
auto-sync hook, and all 18 uow_action decorators are deleted; the
InlineElement struct itself stays as an internal transient view type
used by readers).

Comparison against `baseline-pre-rope` tag captured before the rope
migration started.

| Scenario | Pre-rope | Phase 1 WIP (1.13e) | Post-1.14 | Δ vs pre-rope |
|---|---|---|---|---|
| A. 3 KB markdown (undo cleared) | 451.51 KiB | ~450 KiB | **132.82 KiB** | −70% (3.4× lower) |
| B. + 10 char inserts (undo kept) | 481.30 KiB | ~474 KiB | **143.13 KiB** | −70% (3.4× lower) |
| C. select-all+paste (undo kept) | 1.08 MiB | 1.15 MiB | **368.21 KiB** | −67% (3.0× lower) |
| D. 2× content directly | 698.34 KiB | 849.07 KiB | **267.95 KiB** | −62% (2.6× lower) |
| E. empty doc floor | 77.06 KiB | 80.11 KiB | **68.84 KiB** | −11% |
| F. 1 MB plain text | 1.98 MiB | 1.99 MiB | **1.02 MiB** | −48% (1.9× lower) |
| G. 100×1 KB blocks | 1008.95 KiB | 1.07 MiB | **513.97 KiB** | −49% (2.0× lower) |
| H. 3 KB with bold every 5 chars | 1.55 MiB | 1.81 MiB | **250.81 KiB** | **−84% (6.3× lower)** |
| I. 100-cell table | 1.44 MiB | 1.55 MiB | **1.19 MiB** | −17% |
| J. 100 KB doc + 1000 inserts (undo kept) | 192.80 MiB | 192.80 MiB | **96.64 MiB** | −50% (2.0× lower) |

The 6.3× win on **H** is the headline: a heavily-formatted 3 KB doc
had 600 InlineElement entries (one per bold span) under the old
model. Those entries lived in the inline_elements table, the
junction, and were duplicated into every undo snapshot via
`HashMapStoreSnapshot.inline_elements`. Phase 1.14 dropped all three;
the same formatting now lives as 600 `FormatRun`s in a single
`Vec<FormatRun>` per block, sharing structure via `im::HashMap` on
clone.

The wins on **A/B** (3.4×) and **C** (3.0×) come from the same
mechanism applied to ordinary markdown content: small documents
were dominated by the per-element entity overhead, which is now
gone.

**J** (1000 small inserts, undo kept) halves because every undo
snapshot used to clone the full `inline_elements` HashMap; now it
doesn't. The remaining 96 MiB is the snapshot stack itself — each
snapshot still clones `blocks` and the various junction maps. That
floor will drop further when Phase 2 (rope) replaces the per-block
plain_text strings with a shared rope.

**E/I** show smaller wins because the empty-doc floor and table
overhead are dominated by structural entities (frames, table cells)
that 1.14 doesn't touch.

## What 1.14 deleted

- `HashMapStore.inline_elements` table + matching snapshot field
- `HashMapStore.jn_inline_element_from_block_elements` junction +
  matching snapshot field
- `crates/{common,direct_access}/src/.../inline_element/` directories
  (controllers, repositories, DTOs, units_of_work) — ~1630 lines of
  Qleany-generated scaffolding
- 18 `#[macros::uow_action(entity = "InlineElement", action = …)]`
  decorators across 9 use cases + 9 UoWs
- `Block.elements: Vec<EntityId>` field + every `elements: vec![]`
  initializer (~12 sites)
- `BlockRelationshipField::Elements` variant + match arms in
  block_table.rs / block_repository.rs
- `DirectAccessEntity::InlineElement` + `FlatEventKind::
  InlineElement{Created,Updated,Removed}` variants and arms
- Auto-sync hook `sync_block_format_runs` (was inside
  `inline_element_repository::*`; deleted with the dir)
- The `format_runs_dual_write_tests` suite (no longer applicable)

## What survives (for now)

- `InlineElement` struct + `InlineContent` enum in
  `common::entities` — internal transient view type used by readers
  (`extract_fragment_uc`, `export_{html,markdown,latex,docx}_uc`,
  `get_text_at_position_uc`, `text_block.fragments()`,
  `cursor.char_format()`) that haven't yet been rewritten to walk
  `(format_runs, block_images)` directly. Phase 2's rope migration
  naturally subsumes this rewrite.
- `synthesize_block_inline_elements` in `format_runs_query` — the
  single readers' bridge; produces a transient `Vec<InlineElement>`
  view from format_runs + block_images for the readers above.
- `rebuild_block_inline_elements` / `drop_block_inline_elements` —
  `#[deprecated]` no-op stubs. The 12 writer call sites that
  invoked them during the dual-write era still compile; deletion of
  those call sites is a mechanical follow-up.
- `qleany.yaml` still declares the InlineElement entity. The
  generated code no longer references it, but the YAML is the
  source of truth for re-generation. A YAML cleanup pass will
  re-align it before the next Qleany run.

## Phase 1 status

- Phase 0 (baseline): complete ✓
- Phase 1.1–1.12 (foundations + auto-sync): complete ✓
- Phase 1.13a–1.13e (reader + writer migrations): complete ✓
- **Phase 1.14a (table + junction + decorator deletion): complete ✓**
- Phase 1.14b (follow-ups: drop deprecated bridge calls, drop
  InlineElement struct, qleany.yaml realignment): pending,
  non-blocking
- Phase 1.17 (criterion bench compare): pending, scheduled

Phase 2 (rope swap) is now unblocked. Phase 2 naturally subsumes
the surviving InlineElement struct + reader-bridge by rewriting the
readers to walk a ropey-backed structure directly.

## Known pre-existing bug (unchanged from 1.13e)

The cursor::finish_edit_ext overflow surfaced during 1.13e proptest
runs (seed `9f9c3b14…`) still reproduces on pre-rope and post-1.14
baselines alike. Out of scope here; tracked separately.
