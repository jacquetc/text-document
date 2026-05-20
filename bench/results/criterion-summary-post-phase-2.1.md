# Criterion: pre-rope vs post-phase-2.1

`feature/rope-backend` HEAD after Phase 2.1 fix #1 (Cause A —
`BlockOffsetIndex::range_of` O(n) → O(1) via parallel
`marker_index: im::HashMap<OffsetMarker, usize>` cache).

Raw output: [criterion-pre-rope-vs-post-phase-2.1.txt](criterion-pre-rope-vs-post-phase-2.1.txt)

## Overall vs pre-rope baseline

| Category | Phase-2 | **Phase-2.1** |
|---|---|---|
| Improved (significant, Δ ≤ −2 %) | 62 | **65** |
| Within noise | 7 | **7** |
| Regressed (significant, Δ ≥ +2 %) | 58 | **55** |
| **Total benches** | 127 | 127 |

Net: 3 benches flipped from regressed/noise to improved. Headline
shape is unchanged — Phase 2.1 fix #1 alone is a modest win on
the overall bench count.

## Wins delivered by fix #1

The fix targeted Cause A (write-side `range_of` lookups). The five
large-doc insertion benches were the headline target, and all five
moved decisively:

| Bench | Phase-2 vs pre-rope | **Phase-2.1 vs pre-rope** | Δ-of-Δ |
|---|---|---|---|
| `insertion/insert_char_at_start/large/1000para` | +9.8 % | **−41.7 %** | **−51.5 pts** ✓ |
| `insertion/insert_paragraph/large/1000para` | +45.5 % | **+2.8 %** (noise) | **−42.7 pts** ✓ |
| `insertion/insert_char_at_middle/large/1000para` | +33.1 % | **−3.3 %** (improved) | **−36.4 pts** ✓ |
| `insertion/insert_word/large/1000para` | +37.6 % | **+2.9 %** (noise) | **−34.7 pts** ✓ |
| `insertion/insert_char_at_end/large/1000para` | +66.8 % | **+39.9 %** | **−26.9 pts** ✓ |

The smaller wins on small-doc I/O benches are also real:

| Bench | Phase-2 | **Phase-2.1** | Δ-of-Δ |
|---|---|---|---|
| `plain_text_io/to_plain_text/small/1para` | +11.1 % | −0.7 % | −11.8 pts |
| `markdown_io/set_markdown/small/1para` | +6.4 % | −1.1 % | −7.5 pts |
| `markdown_io/set_markdown/medium/100para` | +6.1 % | −0.9 % | −7.0 pts |
| `editing_session/mixed_operations_30_large` | +80.4 % | **+73.6 %** | −6.8 pts |
| `html_io/set_html/small/1para` | +5.7 % | −0.9 % | −6.7 pts |
| `html_io/set_html/medium/100para` | −0.5 % | −6.9 % | −6.3 pts |

## Cost: new regressions introduced by the fix

The eager `im::HashMap` maintenance on `insert_at`/`remove_at` adds
O(N log N) HAMT path-copies per call (where the original Vec was
O(N) shifts). For medium-doc workloads this cost shows up against
a small base:

| Bench | Phase-2 | **Phase-2.1** | Δ-of-Δ | Verdict |
|---|---|---|---|---|
| `plain_text_io/to_plain_text/large/1000para` | +1.6 % | +18.1 % | +16.5 pts | **real regression** |
| `insertion/insert_char_at_middle/medium/100para` | −29.2 % | −13.6 % | +15.6 pts | still improved vs pre-rope |
| `insertion/insert_char_at_start/medium/100para` | −45.3 % | −31.2 % | +14.1 pts | still improved |
| `insertion/insert_char_at_end/medium/100para` | −21.1 % | −7.3 % | +13.8 pts | still improved |
| `insertion/insert_block/medium/100para` | −44.4 % | −30.9 % | +13.5 pts | still improved |
| `insertion/insert_word/medium/100para` | −29.8 % | −18.6 % | +11.2 pts | still improved |
| `deletion/delete_selection/medium/100para` | −20.4 % | −10.0 % | +10.4 pts | still improved |
| `formatting/merge_char_format_bold` | −27.1 % | −17.2 % | +9.9 pts | still improved |
| `formatting/insert_formatted_text` | −48.8 % | −40.4 % | +8.4 pts | still improved |
| `undo_redo/undo_single` | +14.4 % | +19.9 % | +5.5 pts | worse |
| `undo_redo/redo_single` | +13.3 % | +19.3 % | +6.0 pts | worse |
| `deletion/delete_char_backward/large/1000para` | +82.9 % | +89.7 % | +6.8 pts | worse |

Most of the "worsenings" above are still big improvements vs
pre-rope — they just lost some of the headroom that Phase 2 had.
The real concern is the +16.5-pt regression on `to_plain_text/large`
and the +5–7-pt drift on `undo_redo/{undo,redo}_single`.

## Unchanged regressions (not Cause A)

The benches that dominated the regression list are mostly tied to
Causes B (read-side use cases walk the block tree) and G
(`block_char_length` is O(L)), neither of which fix #1 addresses.
They are essentially flat between phase-2 and phase-2.1:

| Bench | Phase-2 | **Phase-2.1** | Cause |
|---|---|---|---|
| `document_queries/block_at_position/large/1000para` | +156 % | **+162 %** | B + G |
| `xl/set_bold_on_100kb_selection` | +155 % | **+160 %** | F |
| `cursor_movement/select_word/large/1000para` | +151 % | **+151 %** | B + G |
| `document_queries/text_at/large/1000para` | +144 % | **+148 %** | B + G |
| `document_queries/block_at_position/medium/100para` | +85 % | **+90 %** | B + G |
| `cursor_movement/select_block/large/1000para` | +79 % | **+79 %** | B + G |
| `editing_session/mixed_operations_30_large` | +80 % | **+74 %** | mixed |
| `document_queries/blocks_in_range/large/1000para` | +69 % | **+72 %** | B |
| `snapshot/snapshot_block_at_position/large/1000para` | +44 % | **+47 %** | B + D |
| `search/find_regex/large/1000para` | +39 % | **+42 %** | E |

These are exactly the benches the
[regression review](regression-review-phase-2.md) flagged as needing
fix B (read-side use case rewrite), fix G
(`block_char_length` via rope index), fix F (hand-rolled inverse for
`set_text_format`), and fix E (`find_*` rope.chunks fast path).
None of them can be moved by Cause A alone.

## What the data tells us

Fix #1's hypothesis was that `range_of` O(1) would cascade
improvements through every block-id lookup. **The hypothesis is half
right.** It moves the write-side benches strongly (the five
large-doc insertion benches are now in the win column) but does
*not* move the read-side benches at all — because the read-side use
cases (`get_block_at_position_uc`, `get_text_at_position_uc`,
`find_word_boundaries`) never call `range_of`. They walk the block
tree and call `block_char_length` per block, which is the
`block_content_via_store(...).chars().count()` path — independent
of the offset index.

This is **Cause B's territory**, not Cause A's. The
regression review attributed too many benches to Cause A and not
enough to Cause B.

Updated forecast (revised based on phase-2.1 data):

| Fix | What it actually moves | Estimated cumulative improvement |
|---|---|---|
| **A — `range_of` O(1)** (landed) | 5 large-insertion + several small I/O | 62 → 65 improved |
| **G — `block_char_length` via rope index** | `document_queries/*`, `cursor_movement/select_*`, `snapshot_block_at_position` (the read-side hotpath) | ~10–15 more benches recovered |
| **B — rewrite `get_block_at_position` + `get_text_at_position`** | Same as G but bigger gains; the use case becomes O(log n) end-to-end | flips many of the +50 to +160 % rows to wins |
| **F — hand-rolled inverse for set_text_format** | `xl/set_bold_on_100kb_selection`, `formatting/set_text_format/*` | resolves the +160 % regression |
| **E — `find_*` via `rope.to_string()`** | `search/find_*`, delivers plan §5.4 target | drops +15-40 % search regressions |
| **C — `shift_after` binary search** | per-op write paths on large/1000para deletion/insertion | recovers `deletion/delete_char_*/large` |
| **D — snapshot CoW for offset index** | `undo_redo/*`, `snapshot/snapshot_*` | resolves the +6 pt drift introduced by fix #1 |

The biggest near-term leverage shifted from A → **G** (smallest fix,
biggest impact across the read-side benches).

## Memory profile

Unchanged from phase-2 — the `marker_index` cache adds one
`im::HashMap<OffsetMarker, usize>` per `BlockOffsetIndex`. Each
entry is ~24 bytes; on a 1000-block doc that's ~24 KiB of additional
heap. Negligible against the 1.13 MiB total for the 1 MB plain-text
case in scenario F.

(Memory profile not re-captured for this run; the structural change
is small and predictable.)

## Recommendation

- **Keep fix #1**: net positive on bench count, decisive on the five
  large-insertion benches, cleans up small-doc I/O regressions, no
  test regressions.
- **Land fix G next** (`block_char_length` via rope index, ~10 LOC).
  Smallest possible code change with the biggest expected impact on
  the read-side hotpath — should move many of the +50 to +160 %
  regressions toward zero.
- **Then fix B** (read-side use case rewrites). Bigger code change
  but the natural follow-on to G.
- **Defer fix D** (snapshot CoW) until after B+G land — the +6 pt
  drift on `undo_redo/{undo,redo}_single` is small enough that it
  may not justify the complexity.
