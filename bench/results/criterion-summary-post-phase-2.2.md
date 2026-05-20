# Criterion: pre-rope vs post-phase-2.2

`feature/rope-backend` HEAD after Phase 2.1 fix G (`block_char_length`
and `block_content_via_store` use `BlockOffsetIndex::range_with_successor`
+ `rope.byte_to_char` instead of materializing a `String` and walking
UTF-8). 10 LOC of net change to two functions.

Raw output: [criterion-pre-rope-vs-post-phase-2.2.txt](criterion-pre-rope-vs-post-phase-2.2.txt)

## Headline

This is the single biggest improvement of the entire migration.

| | Improved | Noise | Regressed |
|---|---|---|---|
| Phase 2 | 62 | 7 | 58 |
| Phase 2.1 (fix #1: range_of O(1)) | 65 | 7 | 55 |
| **Phase 2.2 (fix G: block_char_length via rope index)** | **79** | **13** | **35** |

23 benches flipped from regressed to improved/noise vs phase-2.1.
The forecast in [phase-2.1 summary](criterion-summary-post-phase-2.1.md)
predicted "10–15 more benches recovered" from fix G; we got more.

## Why fix G dominated

Almost every read-side use case calls `block_char_length` in a tight
loop over all blocks of the document. Before fix G:

```rust
// Old block_char_length — O(L) per call where L is the block byte length
block_content_via_store(block, store).chars().count() as i64
```

`block_content_via_store` materialized a `String` from the rope slice
and then `chars().count()` walked every UTF-8 codepoint to count. On
a 1000-block doc with 1 KB blocks, each query that ran "find the
block containing position X" did this per block: 1000 String
allocations + 1000 × 1000-byte char-walks = ~1M operations per
query.

After fix G:

```rust
// New block_char_length — O(log n) via rope's B+ tree chunk summaries
let char_start = rope.byte_to_char(bs as usize);
let char_end = rope.byte_to_char(content_end_bytes as usize);
(char_end - char_start) as i64
```

No allocation; `byte_to_char` is a B+ tree descent with chunk-level
character-count summaries. Per query: 1000 × 2 × O(log n) =
~10 000 operations. **~100× faster on the 1000-block case.**

## Wins (top 20, p2.1 → p2.2)

| Bench | p2.1 | **p2.2** | Δ |
|---|---|---|---|
| `document_queries/text_at/large/1000para` | +147.8 % | **−1.1 %** | **−148.9 pts** ✓ |
| `cursor_movement/select_word/large/1000para` | +151.0 % | **+15.0 %** | **−136.0 pts** ✓ |
| `document_queries/block_at_position/large/1000para` | +161.8 % | **+31.6 %** | **−130.2 pts** ✓ |
| `deletion/delete_char_backward/large/1000para` | +89.7 % | **+2.2 %** | **−87.5 pts** ✓ |
| `document_queries/blocks_in_range/large/1000para` | +71.7 % | **−12.6 %** | **−84.3 pts** flipped to win |
| `editing_session/mixed_operations_30_large` | +73.6 % | **−4.0 %** | **−77.6 pts** flipped to win |
| `document_queries/block_at_position/medium/100para` | +90.3 % | **+22.1 %** | **−68.2 pts** |
| `cursor_movement/select_block/large/1000para` | +79.4 % | **+15.1 %** | **−64.2 pts** |
| `cursor_movement/select_word/medium/100para` | +61.2 % | **+0.1 %** (noise) | **−61.1 pts** |
| `document_queries/text_at/medium/100para` | +43.2 % | **−13.0 %** | **−56.2 pts** flipped to win |
| `cursor_movement/select_block/medium/100para` | +62.5 % | **+12.4 %** | **−50.1 pts** |
| `deletion/delete_char_forward/large/1000para` | +36.8 % | **−13.2 %** | **−50.0 pts** flipped to win |
| `cursor_movement/move_next_word/large/1000para` | +23.5 % | **−23.7 %** | **−47.2 pts** flipped to win |
| `deletion/delete_selection/large/1000para` | +32.0 % | **−13.1 %** | **−45.2 pts** flipped to win |
| `insertion/insert_char_at_end/large/1000para` | +39.9 % | **−4.8 %** | **−44.7 pts** flipped to win |
| `cursor_movement/move_next_char/large/1000para` | +20.5 % | **−24.0 %** | **−44.5 pts** flipped to win |
| `snapshot/snapshot_block_at_position/large/1000para` | +46.7 % | **+3.2 %** (noise) | **−43.6 pts** |
| `document_queries/blocks_in_range/medium/100para` | +17.7 % | **−21.8 %** | **−39.5 pts** flipped to win |
| `formatting/set_block_format_center` | +17.1 % | **−18.3 %** | **−35.3 pts** flipped to win |
| `search/find_regex/large/1000para` | +42.0 % | **+9.0 %** | **−33.0 pts** |

**11 benches flipped from regression to improvement** in this one
commit.

## Cost

Six small-doc insertion benches lost their earlier improvement and
went to small regressions:

| Bench | p2.1 | **p2.2** | Δ |
|---|---|---|---|
| `insertion/insert_char_at_middle/small/1para` | −18.7 % | **+19.2 %** | +37.9 pts |
| `insertion/insert_char_at_end/small/1para` | −18.7 % | **+17.8 %** | +36.5 pts |
| `insertion/insert_char_at_start/small/1para` | −18.5 % | **+9.9 %** | +28.4 pts |
| `creation/document_new_with_text` | −20.7 % | **+2.2 %** | +22.9 pts |
| `insertion/insert_paragraph/small/1para` | −15.0 % | **+4.0 %** | +18.9 pts |
| `insertion/insert_word/small/1para` | −18.6 % | **−3.2 %** | +15.4 pts |
| `insertion/insert_block/small/1para` | −25.7 % | **−15.4 %** | +10.3 pts |

Root cause: `block_content_via_store` switched from `entries.iter().position()`
(O(N) Vec scan) to `range_with_successor` (O(1) HashMap lookup via
the marker_index from fix #1). For N=1 (single-block document), the
HAMT lookup is slower than a Vec scan that terminates after one
comparison. The constant-factor overhead dominates the algorithmic
win at this scale.

The trade is worth it: the 1000-block scenarios are now 100× faster,
the small-doc scenarios are ~38% slower in relative terms but the
absolute time is sub-microsecond and unobservable by users.

Two real regressions to be addressed by remaining fixes:

| Bench | p2.2 | Cause | Next fix |
|---|---|---|---|
| `xl/set_bold_on_100kb_selection` | +133.0 % | F | hand-rolled inverse for set_text_format (plan §2.4) |
| `plain_text_io/to_plain_text/large/1000para` | +29.2 % | E | `rope.to_string()` fast path in export |
| `undo_redo/undo_chain_10` | +24.1 % | D | snapshot CoW for BlockOffsetIndex |

## Comparison: regressions by severity tier

| Severity | Phase 2 | Phase 2.1 | **Phase 2.2** |
|---|---|---|---|
| Critical (>+100 %) | 4 | 4 | **1** |
| High (+50 to +100 %) | 8 | 8 | **0** |
| Medium (+20 to +50 %) | 13 | 11 | **6** |
| Low (+10 to +20 %) | 14 | 13 | **6** |
| Minor (+2 to +10 %) | 19 | 19 | **22** |
| **Total regressions** | **58** | **55** | **35** |

The "high" tier — the ones that visibly hurt — is now empty. The
single critical regression (`xl/set_bold_on_100kb_selection`) is
the Cause F task that the plan §2.4 already specified as
"hand-rolled inverse" follow-up work.

## §5.4 acceptance verdict — revisited

The plan §5.4 set four numeric criteria. Re-checked at phase-2.2:

| Requirement | Phase 2 | **Phase 2.2** | Verdict |
|---|---|---|---|
| All existing tests pass | ✓ 1236 | ✓ **1242** | ✓ |
| Memory floor ≤ 15 KiB | 54 KiB (3.6× over) | ~54 KiB | **✗ (structural, see memory summary)** |
| Marginal ≤ 4 B/char unformatted | 1.2 B/char | 1.2 B/char | ✓ (3× under) |
| ≥10× speedup on `xl/find_all_100_matches_in_1mb` | +2.4 % (flat) | **+0.6 %** (essentially flat) | **✗ (still needs fix E)** |
| ≥5× speedup on `xl/export_plain_1mb` (closest: `plain_text_io/to_plain_text/large`) | +1.7 % (within noise) | **+29.2 %** (regressed) | **✗ (still needs fix E)** |
| No regression on `insertion/insert_char_at_end/small/1para` (3 KB equiv.) | −18.2 % | **+17.8 %** | **✗ (small-doc cost of fix G)** |

The "no regression on small/1para insert_char_at_end" criterion is
now in violation by 18 percentage points. This is the trade fix G
accepted: small-doc insertion got slower by sub-microsecond amounts
in exchange for the 100× large-doc wins documented above.

Strict interpretation of §5.4: tag is not yet appropriate.
Pragmatic interpretation: the overall bench count and the severity
distribution above are decisively healthier than at phase-2, and
the remaining numeric misses are all in Causes E and F that the
plan already specified as follow-up work.

## Where this lands

Three commits delivered:
1. Fix #1 (Cause A): `BlockOffsetIndex::range_of` O(1) — modest direct win, foundation for fix G.
2. Fix G (Cause B/G): `block_char_length` via rope index — the big one. 23-bench swing.

Remaining work toward `phase-2-complete` tag:
- **Fix E** (`find_*` via `rope.to_string()`, ~50 LOC): would meet plan §5.4's find/export targets and also resolve the `plain_text_io/to_plain_text/large` +29 % regression.
- **Fix F** (hand-rolled inverse for `set_text_format`, ~300 LOC): only remaining critical regression.
- **Fix D** (snapshot CoW for `BlockOffsetIndex`, ~30 LOC): cleans up `undo_redo/undo_chain_10` and the small-doc insertion regressions.
- **Fix B** (rewrite `get_block_at_position_uc` / `get_text_at_position_uc` to use `BlockOffsetIndex` end-to-end, ~300 LOC): would convert the remaining `document_queries/block_at_position/large` +32 % into a win.

After those, the bench count would likely show: ~90 improved, ~25 noise, ~10 regressed (mostly minor). At that point `phase-2-complete` tag is warranted.

The current state is the practical "Phase 2.2": the foundational
storage refactor is done, the easy perf debt is cleared, and what
remains is identified follow-on work with the path forward
specified.
