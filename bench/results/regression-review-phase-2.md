# Per-regression review — Phase 2

Walking the 58 regressed benches from
[criterion-summary-post-phase-2.md](criterion-summary-post-phase-2.md),
tracing each one to its actual code path, and proposing concrete fixes.

Causes are labeled A–H so they can be referenced from the per-bench
table. Fixes are ordered by leverage (how many benches the same fix
moves at once).

---

## Causes (root catalogue)

### Cause A — `BlockOffsetIndex::range_of` is O(n)

[block_offset_index.rs:113-122](file:///Users/cyril/Devel/text-document/crates/common/src/database/block_offset_index.rs#L113-L122):

```rust
pub fn range_of(&self, marker: OffsetMarker) -> Option<(u32, u32)> {
    let idx = self.entries.iter().position(|(m, _)| *m == marker)?;  // O(n)
    ...
}
```

Every caller that has a `block_id` and wants its byte range pays one
linear scan per call. On 1000-block docs that's 1000 entries scanned
per call — and many use cases call it 2–4 times per block.

**Fix**: maintain a parallel `im::HashMap<OffsetMarker, usize>`
giving the position of each marker in `entries`. Update on
`insert_at` / `push` / `remove_at`. `shift_after` doesn't move
positions (only byte_starts), so the index map is unaffected.

**Effort**: ~60 LOC + tests. Low risk: every call site already goes
through `range_of` / `range_of_block`, no API change.

### Cause B — Use cases ignore `BlockOffsetIndex`

`get_block_at_position_uc` and `get_text_at_position_uc` still walk
the block tree linearly, summing `block_char_length` for each block,
to find the block containing a position. The rope migration was
**not propagated into the read-side use cases** — they still operate
in "scan and sum" mode like the pre-rope `String`-per-block model.

Example: [get_block_at_position_uc.rs:71-101](file:///Users/cyril/Devel/text-document/crates/document_inspection/src/use_cases/get_block_at_position_uc.rs#L71-L101)
walks every block, calls `block_char_length(&block, &store)` **four
times per block** (lines 75, 84, 90, 95), and on the fallback path
does **another** full walk (lines 109-114) — so a 1000-block doc
with the position at the end does ~8000 char_length calls.

**Fix**: rewrite these use cases to use the rope index:
1. `byte = rope.char_to_byte(position)` — O(log n)
2. `(block_id, byte_in_block) = block_offsets.byte_to_block_byte(byte)` — O(log n) once cause A is fixed
3. `block_start_char = rope.byte_to_char(byte_range.0)` — O(log n)
4. `block_length_chars = rope.byte_slice(byte_range).len_chars()` — O(log n)

Total per call: 4 × O(log n) instead of 8000 × O(L) chars.

**Effort**: ~100 LOC per use case + tests. Medium risk: must preserve
exact position semantics across block separators, empty blocks, and
the "position at very end" edge case that already has special
handling.

### Cause C — `BlockOffsetIndex::shift_after` is O(N)

[block_offset_index.rs:176-183](file:///Users/cyril/Devel/text-document/crates/common/src/database/block_offset_index.rs#L176-L183):

```rust
pub fn shift_after(&mut self, threshold: u32, delta: i32) {
    for (_, bs) in self.entries.iter_mut() {
        if *bs >= threshold { *bs = apply_delta(*bs, delta); }
    }
    self.total_bytes = apply_delta(self.total_bytes, delta);
}
```

Every per-op edit calls `shift_after` — even for a single-char
insert. On 1000 blocks, that's 1000 entries scanned per edit.

**Fix**: since `entries` are sorted by `byte_start`, binary-search
for the first entry with `byte_start >= threshold`, then iterate from
there to end. For inserts near the end of the doc, that's O(log n +
small_k).

**Effort**: ~20 LOC. Low risk.

A more advanced fix (Fenwick / segment tree of deltas) makes
`shift_after` itself O(log n) but requires reworking the read path
too; not worth it unless C alone is insufficient.

### Cause D — Snapshot clones `BlockOffsetIndex.entries` Vec

Each snapshot allocates a fresh Vec of all `(OffsetMarker, u32)`
entries. On 1000 blocks, that's 16 KB per snapshot. The 1000-undo
stack therefore holds 16 MB of duplicated offset data on top of the
shared rope.

**Fix**: wrap `entries` in `Arc<Vec<...>>` with copy-on-write via
`Arc::make_mut`, **or** switch to `im::Vector` (B-tree backed,
O(log n) clone, O(log n) update). The first is simpler if writes
typically rebuild the whole vec.

**Effort**: ~30 LOC + measurement to confirm CoW is cheaper than
`im::Vector` for this workload. Medium risk: `Arc::make_mut` calls
already exist for the structural HAMTs, the pattern is well-trodden.

### Cause E — `find_*` walks blocks instead of `rope.to_string()`

[find_text_uc.rs:64](file:///Users/cyril/Devel/text-document/crates/document_search/src/use_cases/find_text_uc.rs#L64) calls
`build_full_text_via_store(&blocks, &store)` which does per-block
`block_content_via_store(block, store).chars()` materialization.
For 1000 blocks of 1 KB each = 1000 rope slices + 1000 Cow→String
materializations.

The plan §6.2 promised `rope.chunks()` iteration to replace this;
not landed.

**Fix**: for documents with no nested frames or table cells (the
common case), use `store.rope.read().to_string()` directly — one
allocation, one walk. For tabled docs, keep the per-block walk
(table cell content lives in separate rope ranges that don't appear
in the main flow's contiguous byte range, so the per-block path is
needed).

Detection: check `Document.frame_ids.len() == 1 && frame.child_order
contains no negative entries` → use the fast path.

**Effort**: ~50 LOC + a small unit test. Low risk: the slow path
remains as a fallback for tabled docs.

### Cause F — `set_text_format` uses full snapshot

`xl/set_bold_on_100kb_selection` regressed +154 % because formatting
a 100 KB selection inside one block clones the full
`RopeStoreSnapshot` (rope is Arc-shared so cheap, but the 11
structural HAMTs each get a write-path clone). Plan §2.4 already
classifies this op as **hand-rolled inverse** territory — capture
`(block_id, byte_range, prior_format_runs_in_range)`, splice in
new runs, undo by splicing back the captured runs.

**Fix**: rewrite `set_text_format_uc` and `merge_text_format_uc` to
hand-rolled inverse, as the plan §2.4 prescribed. Drop the full
snapshot in these two use cases.

**Effort**: ~150 LOC each + careful tests around the splice/coalesce
boundary. Medium risk: undo correctness on overlapping selections
needs proptest coverage. The plan §2.4 listed this as the planned
shape, so the design is already specified.

### Cause G — `block_char_length` is O(L) per call

[rope_helpers.rs](file:///Users/cyril/Devel/text-document/crates/common/src/database/rope_helpers.rs):

```rust
pub fn block_char_length(block: &Block, store: &Store) -> i64 {
    block_content_via_store(block, store).chars().count() as i64  // O(L)
}
```

Pre-rope, `Block.text_length` was an O(1) entity field. Post-rope,
removing the field forces every caller to re-count chars by walking
UTF-8. Use cases that call this inside a per-block loop pay
O(N × L) instead of O(N).

**Fix**: `block_char_length` should use the rope index directly:

```rust
pub fn block_char_length(block: &Block, store: &Store) -> i64 {
    let offsets = store.block_offsets.read();
    let (byte_start, byte_end) = match offsets.range_of_block(block.id) {
        Some(r) => r,
        None => return 0,
    };
    let rope = store.rope.read();
    let char_start = rope.byte_to_char(byte_start as usize);
    let char_end = rope.byte_to_char(byte_end as usize);
    (char_end - char_start) as i64
}
```

Both `byte_to_char` calls are O(log n) — much faster than O(L) chars
walked. Fast for short blocks too: the rope's leaf chunk holds the
byte→char tables directly.

After this fix, multi-call sites still pay 2 × O(log n) per call
instead of one — refactoring callers to cache the value is a
secondary win (Cause B's rewrite naturally does this).

**Effort**: ~10 LOC. Low risk.

### Cause H — Small-doc rope materialization overhead

Benches like `plain_text_io/to_plain_text/small/1para` (+11 %),
`cursor_movement/select_*/small` (+7-10 %),
`markdown_io/set_markdown/small` (+6 %), `html_io/set_html/small`
(+6 %) all sit in the +3 to +11 % range on tiny docs.

These are real but small absolute regressions — the doc-creation +
rope-leaf-allocation overhead shows up against a sub-microsecond
base. Not user-visible at any realistic doc count.

**Fix**: probably not worth pursuing in isolation. Some will
naturally improve as Causes A/G are fixed (the per-call setup
overhead drops). If the small-doc regressions still bother us after
A/B/C/G/E land, profile and triage individually.

---

## Per-bench mapping (all 58 regressions)

Sorted by severity. Causes in priority order (the leftmost listed
cause is the dominant one).

### Critical (Δ > +100 %)

| Bench | Δ | Causes | Status after fix |
|---|---|---|---|
| `document_queries/block_at_position/large/1000para` | +156 % | **B**, A, G | likely flips to improvement |
| `xl/set_bold_on_100kb_selection` | +154 % | **F** | drops to ~−20 % once hand-rolled inverse lands |
| `cursor_movement/select_word/large/1000para` | +150 % | **B** (via select→get_block→get_text→inline_segments), A, G | flips to improvement |
| `document_queries/text_at/large/1000para` | +144 % | **B**, A, G | flips to improvement |

### High (Δ +50–100 %)

| Bench | Δ | Causes |
|---|---|---|
| `document_queries/block_at_position/medium/100para` | +85 % | B, A, G |
| `deletion/delete_char_backward/large/1000para` | +83 % | C, position-refresh O(N) loop |
| `editing_session/mixed_operations_30_large` | +80 % | compound: C + A + B across 30 ops on /large/ |
| `cursor_movement/select_block/large/1000para` | +79 % | B (via select→get_block), A, G |
| `document_queries/blocks_in_range/large/1000para` | +69 % | scans all entries; should use byte→marker binary search |
| `insertion/insert_char_at_end/large/1000para` | +67 % | C (shift_after), D (snapshot vec clone) |
| `cursor_movement/select_word/medium/100para` | +63 % | B, A, G |
| `cursor_movement/select_block/medium/100para` | +63 % | B, A |

### Medium (Δ +20–50 %)

| Bench | Δ | Causes |
|---|---|---|
| `insertion/insert_paragraph/large/1000para` | +45 % | C, D |
| `snapshot/snapshot_block_at_position/large/1000para` | +44 % | A, B |
| `document_queries/text_at/medium/100para` | +39 % | B, A, G |
| `search/find_regex/large/1000para` | +39 % | E (per-block walk + slice materialization) |
| `insertion/insert_word/large/1000para` | +38 % | C, D |
| `document_queries/blocks_in_range/small/1para` | +37 % | A (range_of called in tight loop) |
| `deletion/delete_char_forward/large/1000para` | +34 % | C, position-refresh O(N) |
| `insertion/insert_char_at_middle/large/1000para` | +33 % | C, D |
| `undo_redo/undo_chain_10` | +32 % | D (BlockOffsetIndex Vec clone × 10 snapshots restored) |
| `deletion/delete_selection/large/1000para` | +28 % | C, position-refresh O(N) |
| `cursor_movement/move_next_word/large/1000para` | +22 % | B (per-cursor-move position lookup) |
| `document_queries/text_at/small/1para` | +20 % | B, G (overhead visible on tiny doc) |
| `cursor_movement/move_next_char/large/1000para` | +20 % | B |

### Low (Δ +10–20 %)

| Bench | Δ | Causes |
|---|---|---|
| `search/find_first/large/1000para` | +17 % | E |
| `search/find_all/large/1000para` | +16 % | E |
| `document_queries/blocks_in_range/medium/100para` | +16 % | A |
| `search/find_all_case_insensitive/large/1000para` | +16 % | E |
| `undo_redo/undo_single` | +14 % | D |
| `undo_redo/redo_single` | +13 % | D |
| `snapshot/snapshot_block_at_position/medium/100para` | +13 % | A |
| `document_queries/block_at_position/small/1para` | +12 % | B, G (overhead on tiny doc) |
| `plain_text_io/to_plain_text/small/1para` | +11 % | H |
| `cursor_movement/select_word/small/1para` | +10 % | B, G |
| `formatting/set_block_format_center` | +10 % | D (full snapshot still used for block-format) |
| `deletion/delete_char_backward/medium/100para` | +10 % | C, position-refresh |
| `insertion/insert_char_at_start/large/1000para` | +10 % | C, D |
| `markdown_io/to_markdown/large/1000para` | +9 % | per-block emit + rope slice materialization |

### Minor (Δ +2–10 %)

| Bench | Δ | Causes |
|---|---|---|
| `xl/set_plain_text_1mb` | +8 % | rope-build cost on 1 MB input (loading into B+ tree) |
| `cursor_movement/move_to_end/large/1000para` | +8 % | B |
| `cursor_movement/select_block/small/1para` | +8 % | B (overhead on tiny doc) |
| `cursor_movement/move_to_start/large/1000para` | +7 % | B |
| `cursor_movement/move_next_block/large/1000para` | +7 % | B |
| `document_queries/stats/large/1000para` | +7 % | stats sums per-block char_length (G) |
| `search/find_all_case_insensitive/small/1para` | +7 % | E (Cow materialization on tiny doc) |
| `markdown_io/set_markdown/small/1para` | +6 % | H (import-path rope writes) |
| `search/find_first/small/1para` | +6 % | E, H |
| `markdown_io/set_markdown/medium/100para` | +6 % | H |
| `html_io/set_html/small/1para` | +6 % | H |
| `xl/undo_single_char_on_1mb` | +4 % | within noise; D |
| `editing_session/mixed_operations_30` | +4 % | compound |
| `search/find_first/medium/100para` | +4 % | E |
| `search/find_all/small/1para` | +4 % | E, H |
| `html_io/to_html/large/1000para` | +3 % | per-block emit |
| `search/find_all/medium/100para` | +3 % | E |
| `search/find_all_case_insensitive/medium/100para` | +3 % | E |
| `xl/find_all_100_matches_in_1mb` | +2 % | E (would have been the headline win with rope.chunks fast path) |

---

## Recommended fix order (by leverage)

Land in this order — each fix is independently shippable, and the
later fixes' impact is easier to measure once the earlier ones are
in:

1. **Cause A — `range_of` O(1)** (60 LOC). Touches every block-id
   lookup. Expected to halve or fully revert all benches in the
   "A" column above (most of the critical / high / medium rows).
2. **Cause G — `block_char_length` via rope index** (10 LOC).
   Removes the O(L) tax from every call site. Cleans up small-doc
   regressions and removes the multiplier in Cause B's call sites.
3. **Cause C — `shift_after` binary search** (20 LOC). Fixes per-op
   edits on /large/1000para.
4. **Cause B — rewrite read-side use cases to use BlockOffsetIndex**
   (~300 LOC across 2-3 use cases). Biggest single design alignment
   with the rope model. Should be done after A+G so the index calls
   are cheap.
5. **Cause F — hand-rolled inverse for `set_text_format`** (~300 LOC
   across two use cases). Specified in plan §2.4. Resolves the
   xl/set_bold regression.
6. **Cause E — `find_*` fast path via `rope.to_string()`** (~50 LOC).
   Delivers the plan §5.4 `find_all_100_matches_in_1mb` target.
7. **Cause D — snapshot CoW for `BlockOffsetIndex.entries`** (~30 LOC).
   Should be measured after #1–#6; if /large/ undo benches are still
   in the +10 % range, this lands the rest.
8. **Cause H — small-doc overhead**. Likely resolved as a side-effect
   of #1–#7. Re-measure and triage individually if anything still
   stands out.

Estimated total effort: **~700 LOC across 6 fixes**, each
self-contained.

Acceptance: re-run `bench/compare.sh pre-rope post-phase-2.1` and
confirm:
- Regressed-bench count drops from 58 to <15.
- No regression worse than +20 % remains.
- `xl/find_all_100_matches_in_1mb` shows ≥5× speedup (relaxed from
  the plan's 10× target — single-rope `to_string()` is the
  ceiling).
- `xl/set_bold_on_100kb_selection` flips to improvement.
- Memory deliverables unchanged.

Then tag `phase-2-complete`.
