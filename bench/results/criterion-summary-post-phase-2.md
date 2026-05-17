# Criterion: pre-rope vs post-phase-2

`feature/rope-backend` HEAD after Phase 2 (rope swap, junction collapse,
`Block.plain_text` / `Block.text_length` removal), run against the
`pre-rope` baseline captured before the migration started.

Raw output: [criterion-pre-rope-vs-post-phase-2.txt](criterion-pre-rope-vs-post-phase-2.txt)

## Overall

| Category | Count |
|---|---|
| Improved (significant, Δ ≤ −2 %) | 62 |
| Within noise (not significant **or** \|Δ\| ≤ 2 %) | 7 |
| Regressed (significant, Δ ≥ +2 %) | 58 |
| **Total benches** | **127** |

Bucketed by document size:

| Size | Improved | Regressed | Noise |
|---|---|---|---|
| `small/1para` (~few hundred chars) | 19 | 9 | 4 |
| `medium/100para` (~few KB) | 17 | 12 | 2 |
| `large/1000para` (~50–100 KB across many blocks) | 5 | 27 | 1 |
| `xl/*` (1 MB single-block) | 6 | 2 | 0 |
| ungrouped (`creation`, `formatting`, `tables`, `lists`, …) | 15 | 8 | 0 |

The pattern is sharp: **small and medium docs improve broadly, `xl`
(big single-block) wins decisively, but `large/1000para` (many small
blocks) regresses across the board.** Root cause analysis is in
[§ Regressions](#regressions) below.

## Headline improvements

| Bench | Δ vs pre-rope |
|---|---|
| `markdown_io/set_markdown/large/1000para` | **−63.9 %** |
| `plain_text_io/set_plain_text/large/1000para` | **−63.2 %** |
| `plain_text_io/set_plain_text/medium/100para` | **−58.6 %** |
| `document_queries/blocks/medium/100para` | **−58.5 %** |
| `xl/insert_char_at_end_of_1mb` | **−53.3 %** |
| `document_queries/blocks/large/1000para` | **−51.1 %** |
| `html_io/set_html/large/1000para` | **−50.3 %** |
| `formatting/insert_formatted_text` | **−48.8 %** |
| `xl/insert_1kb_at_mid_of_1mb` | **−47.6 %** |
| `xl/insert_char_at_mid_of_1mb` | **−47.1 %** |
| `lists/insert_list` | **−45.1 %** |
| `tables/insert_table_10x10` | **−39.5 %** |
| `tables/insert_table_row` / `_column` | **−29.6 % / −29.0 %** |
| `creation/document_new` / `_with_text` | **−19.1 % / −22.5 %** |

The 1 MB single-block edit benches (`xl/insert_char_*_1mb`,
`xl/insert_1kb_at_mid_of_1mb`) all halve. That is the rope paying off:
char↔byte conversion is now O(log n) via `ropey::Rope`, and the
character data is no longer duplicated into the per-block `String` plus
the snapshot stack.

The bulk-import paths (`set_plain_text`, `set_markdown`, `set_html`
on `large/1000para`) all win 50–64 %. The structural-creation paths
(`tables/insert_table_*`, `lists/insert_list`) win 29–45 %. The
3 KB-doc cases (`small/1para`) win 13–32 % on insertion.

## Regressions

58 benches regressed significantly. The headline regressions, sorted
by severity:

| Bench | Δ | Notes |
|---|---|---|
| `document_queries/block_at_position/large/1000para` | **+156 %** | hot path; root cause below |
| `xl/set_bold_on_100kb_selection` | **+154 %** | known from Phase 1; partially addressed |
| `cursor_movement/select_word/large/1000para` | **+150 %** | hot path; same root cause |
| `document_queries/text_at/large/1000para` | **+144 %** | same root cause |
| `document_queries/block_at_position/medium/100para` | +85 % | same root cause |
| `deletion/delete_char_backward/large/1000para` | +83 % | rope re-mirror + position-refresh combine badly |
| `editing_session/mixed_operations_30_large` | +80 % | compound effect across 30 ops on a large doc |
| `cursor_movement/select_block/large/1000para` | +79 % | block-id lookup path |
| `document_queries/blocks_in_range/large/1000para` | +69 % | scans all entries to compute range overlap |
| `insertion/insert_char_at_end/large/1000para` | +67 % | per-op rope insert + offset shift on 1000-entry index |
| `cursor_movement/select_word/medium/100para` | +63 % | same as `/large/` smaller |
| `cursor_movement/select_block/medium/100para` | +63 % | |
| `insertion/insert_paragraph/large/1000para` | +45 % | |
| `snapshot/snapshot_block_at_position/large/1000para` | +44 % | block lookup inside snapshot path |
| `document_queries/text_at/medium/100para` | +39 % | |
| `search/find_regex/large/1000para` | +39 % | regex still per-block; rope chunks not yet wired in |
| `insertion/insert_word/large/1000para` | +38 % | |
| `deletion/delete_char_forward/large/1000para` | +34 % | |
| `undo_redo/undo_chain_10` | +32 % | snapshot restore touches every block-offset entry |

Other regressions are in the +2 % to +20 % range and follow the same
pattern.

### Root cause #1 — `BlockOffsetIndex::range_of` is O(n)

The dominant regression source is in [block_offset_index.rs:113-122](file:///Users/cyril/Devel/text-document/crates/common/src/database/block_offset_index.rs#L113-L122):

```rust
pub fn range_of(&self, marker: OffsetMarker) -> Option<(u32, u32)> {
    let idx = self.entries.iter().position(|(m, _)| *m == marker)?;  // O(n)
    let start = self.entries[idx].1;
    let end = self.entries.get(idx + 1).map(|(_, bs)| *bs).unwrap_or(self.total_bytes);
    Some((start, end))
}
```

`range_of` is the workhorse for "given a block id, what's its rope
byte range?" It's called by `byte_to_block_byte`, `range_of_block`,
and indirectly by `block_at_position`, `text_at`, every cursor move
that crosses a block boundary, and every snapshot of a block. On a
1000-block document, each call is a 1000-entry linear scan. The
sister method `marker_at_byte` already uses binary search
([line 142](file:///Users/cyril/Devel/text-document/crates/common/src/database/block_offset_index.rs#L142)),
so the regression is purely an oversight in `range_of`'s
implementation, not a structural limitation.

**Fix (out of scope for this commit, scheduled as a follow-up):**
maintain a parallel `im::HashMap<EntityId, usize>` index from marker
to its position in `entries`, kept in sync with `insert_at` / `push`
/ `remove_at`. That collapses `range_of` to O(1) and naturally
restores every regressed bench in the table above.

A quick scan of the regressed benches that share this root cause:
`document_queries/block_at_position`, `document_queries/text_at`,
`cursor_movement/select_word|block` on `medium`/`large`,
`snapshot/snapshot_block_at_position/large`, the `editing_session`
compound bench, and many of the per-op `insertion`/`deletion` benches
on `large/1000para` — that is, the bulk of the regression list.
Estimated gain from the fix: most of these benches return to within
±10 % of `pre-rope`, several flip to improvements.

### Root cause #2 — `xl/set_bold_on_100kb_selection` (+154 %)

This bench was flagged in [phase-1 summary § "xl/set_bold_on_100kb_selection — root cause and Phase-2 fix"](criterion-summary-post-phase-1.md) and predicted to resolve when Phase 2 replaced the per-block `String` walk with a rope. It did not.

What happened: the char↔byte conversion *is* now O(log n) via `ropey::Rope::char_to_byte` (good), but the `format_runs` splice still walks every byte-ranged run inside the affected block to find the splice point, coalesces with adjacent equal-format runs, and rewrites the `Vec<FormatRun>` for the block. For a 100 KB block holding ~one run, the splice is O(1) work but is wrapped in a snapshot+restore cycle that clones the full RopeStoreSnapshot. The 154 % cost dominates the rope's char_to_byte speedup.

This is structural — the `format_runs` are stored per-block as a flat
`Vec<FormatRun>`, and a 100 KB selection inside a single block forces
the snapshot path on every test iteration. Mitigation requires
either a finer-grained snapshot (skip cloning per-block format_run
vectors that are unchanged) or special-casing pure-format edits to
use a hand-rolled inverse instead of a full snapshot. Both are
non-trivial and not attempted here.

For multi-block real-world workloads (where each block is small),
the symptom does not appear: `formatting/merge_char_format_bold` on
small selections is **−27.1 %**, and `formatting/insert_formatted_text`
is **−48.8 %**.

### Root cause #3 — search benches on large doc

`search/find_*/large/1000para` regressed 15–17 % across the board. The
plan §6.2 anticipated wiring `find` into `ropey::iter::Chunks` to scan
the rope directly, avoiding per-block `String` allocation. That
rewrite was not landed in Phase 2 — `find` still iterates blocks and
calls `block_content_via_store(...)` per block, which slices the rope
into a fresh `String` each time. The 16 % regression is the cost of
the rope-slice overhead (`Cow<str>` materialization + char_indices)
on top of the existing per-block loop.

Plan §6.2 promised `xl/find_all_100_matches_in_1mb_doc` ≥10×
speedup; we measured **+2.4 %** (within noise). The find rewrite is
the remaining work to deliver that target.

### Smaller regressions

- `undo_redo/{undo_single,redo_single,undo_chain_10}` regressed 13–32 %. The snapshot restore path now copies the rope (Arc-shared, O(1)) plus 11 `im::HashMap`s (HAMT-shared, O(log diverged)) plus the BlockOffsetIndex Vec (full clone). On small docs the absolute work is tiny; the regression is the BlockOffsetIndex Vec clone showing up against a small base. Not a structural problem.
- `plain_text_io/to_plain_text/small/1para` +11 %, `cursor_movement/select_*/small` +7–10 %, `markdown_io/set_markdown/small` +6 %, `html_io/set_html/small` +6 %: all small-doc overheads that show up against a tiny base; absolute costs are sub-microsecond and not user-visible.
- `formatting/set_block_format_center` +10 %: block-format set still uses full snapshot; the BlockOffsetIndex clone shows up.
- `xl/set_plain_text_1mb` +8 %: importing a 1 MB string into a fresh doc — the rope build cost (loading into the B+ tree) is real but small.

## §5.4 acceptance verdict

The plan §5.4 set four numeric criteria. Result:

| Requirement | Result | Verdict |
|---|---|---|
| All existing tests pass | 1236 / 1236 (workspace) | ✓ |
| `parallel_backend_test` passes 10 000 ops | n/a — deleted with `HashMapStore`; was used in-flight, not preserved | n/a |
| Memory floor ≤ 15 KiB | empty doc = 54 KiB (E scenario) | **✗ — 3.6× over** |
| Marginal ≤ 4 B/char unformatted | 1 MB plain doc = 1.13 MiB / 1 M chars = **1.2 B/char** | ✓ (3× under) |
| ≥10× speedup on `xl/find_all_100_matches_in_1mb` | +2.4 % (flat) | **✗ — find rewrite not landed** |
| ≥5× speedup on `xl/export_plain_1mb` (closest: `plain_text_io/to_plain_text/large/1000para`) | +1.7 % (within noise) | **✗ — export still per-block** |
| No regression on `insertion/insert_char_at_end/small/1para` (3 KB equivalent) | −18.2 % | ✓ |

Two of the four numeric speedup targets did not land: the `find` and
`export` paths still iterate per-block rather than walking the rope's
B+ tree directly. The memory marginal-per-char target was beaten by
3×, but the empty-doc floor target was missed by 3.6× (structural
entities — Root, Document, root Frame, savepoint store — still cost
54 KiB before any content).

## Where this lands

Phase 2 succeeded as a **storage refactor**:

- The rope is in place. `Block.plain_text` and `Block.text_length` are
  gone. The 12 junction tables collapsed into inline `Vec<EntityId>`
  fields. `HashMapStore` is deleted. The rope-backed snapshot path is
  O(1) for the rope itself and O(log diverged) for the structural
  HAMTs.
- The full undo / redo semantics are preserved (1236 tests pass,
  including the proptest fuzzing suite).
- Memory wins on the data-cost dimension are real and material: the
  bold-every-5-chars 3 KB doc went from 1.55 MiB live to 243 KiB
  (**6.4× lower**), and the 1000-inserts-undo-kept stress case went
  from **192.8 MiB to 885 KiB (218× lower)** (see
  [memory-comparison-post-phase-2.md](memory-comparison-post-phase-2.md)).

Phase 2 left **performance debt** in three concrete places:

1. `BlockOffsetIndex::range_of` is O(n); should be O(1) with a
   parallel id→index map. Fixing this alone is expected to convert
   most `/large/1000para` regressions to improvements.
2. `find_*` use cases still walk blocks instead of `rope.chunks()`.
   Wiring this in delivers the §5.4 `find_all` ≥10× target.
3. `set_bold_on_100kb_selection` requires either snapshot tightening
   (don't clone per-block format_run vectors that didn't diverge)
   or a hand-rolled inverse for pure-format edits.

Recommendation: **do not yet tag `phase-2-complete`**. Land the
three performance items above (tracked as Phase 2.1) and re-run the
benchmark before tagging. The memory deliverables stand regardless.
