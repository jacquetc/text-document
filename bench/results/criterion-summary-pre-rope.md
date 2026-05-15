# Pre-rope baseline (criterion + memory profile)

**Captured**: feature/rope-backend @ b8ad21e (HEAD after XL bench addition)
**Host**: macOS 26.5 (Darwin 25.5.0 arm64), Apple M4, 16 GiB RAM
**Toolchain**: see [rustc-pre-rope.txt](rustc-pre-rope.txt)
**Build**: release, workspace defaults (`lto = "fat"`, `codegen-units = 1`)
**Proptest seed**: 20260515

This file is the curated headline view. Full raw outputs in
[criterion-pre-rope.txt](criterion-pre-rope.txt) and
[memory-pre-rope.txt](memory-pre-rope.txt). Criterion's structured data is in
`target/criterion/*/pre-rope/` (referenced by `cargo bench -- --baseline pre-rope`).

## Memory headlines

| Scenario | Live |
|---|---|
| E. empty doc (TextDocument::new) | **77 KiB** |
| A. 3 KB markdown, undo cleared | 451 KiB |
| F. 1 MB plain text, undo cleared | 1.98 MiB |
| G. 100 blocks × 1 KB | 1009 KiB |
| H. 3 KB doc with bold every 5 chars | 1.55 MiB |
| I. 10×10 empty table | 1.44 MiB |
| **J. 100 KB doc + 1000 small inserts, undo kept** | **192.80 MiB** |

The **J** number is the smoking gun. ~190 KiB per undo step because the
simple-insert hand-rolled inverse clones the full current InlineElement,
and that InlineElement holds the entire 100 KB block text. After rope
migration this should fall to ~tens of bytes per undo step (rope clone
is Arc-shared; only diverged HAMT paths cost memory).

## Critical XL latencies (1 MB workloads)

| Bench | Median | Rope target | Expected speedup |
|---|---|---|---|
| `xl/set_plain_text_1mb` | **430 µs** | ~5–20 µs | ~30–80× |
| `xl/insert_char_at_end_of_1mb` | **1.56 ms** | ~10–50 µs | ~30–150× |
| `xl/insert_char_at_mid_of_1mb` | **944 µs** | ~10–50 µs | ~20–100× |
| `xl/insert_1kb_at_mid_of_1mb` | **1.00 ms** | ~10–30 µs | ~30–100× |
| `xl/undo_single_char_on_1mb` | **1.62 ms** | ~50–200 µs | ~10–30× |
| `xl/undo_redo_cycle_single_char_on_1mb` | **4.08 ms** | ~100–400 µs | ~10–40× |
| `xl/find_all_100_matches_in_1mb` | **6.44 ms** | ~0.5–2 ms | ~3–10× |
| `xl/set_bold_on_100kb_selection` | 64 µs | ~50–200 µs | flat or modest |

The last row is interesting: today's `set_bold` on plain text is already
fast because the document has exactly one InlineElement covering all
100 KB; toggling bold just splits it. After rope migration the work
shifts to format-run splicing, similar latency.

## Existing large-scale benches that should win big

| Bench | Median | Rope target | Expected speedup |
|---|---|---|---|
| `insertion/insert_char_at_end/large/1000para` (130 KB) | **292 µs** | ~30–80 µs | ~5–10× |
| `deletion/delete_char_forward/large/1000para` | **1.96 ms** | ~50–150 µs | ~15–40× |
| `cursor_movement/move_next_char/large/1000para` | **2.07 ms** | ~50–100 µs | ~20–40× |
| `editing_session/mixed_operations_30_large` | **44.5 ms** | ~3–10 ms | ~5–15× |
| `plain_text_io/set_plain_text/large/1000para` | **75.7 ms** | ~1–5 ms | ~15–80× |
| `plain_text_io/to_plain_text/large/1000para` | 1.29 µs | ~1 µs | flat (already trivial) |
| `markdown_io/set_markdown/large/1000para` | 165 ms | ~30–80 ms | parser-bound |
| `html_io/set_html/large/1000para` | 120 ms | ~20–50 ms | parser-bound |
| `snapshot/snapshot_flow/large/1000para` | **3.15 ms** | ~50–200 µs | ~15–60× |

## Small-doc benches (regression watch zone — must NOT slow down)

| Bench | Median | Tolerance |
|---|---|---|
| `creation/document_new` | 23 µs | ±20% |
| `insertion/insert_char_at_end/small/1para` | 9.85 µs | ±20% |
| `deletion/delete_char_forward/small/1para` | 15.9 µs | ±20% |
| `cursor_movement/move_next_char/small/1para` | 14.0 µs | ±20% |
| `search/find_first/small/1para` | 1.73 µs | ±20% |
| `undo_redo/undo_single` | 85 µs | ±30% |
| `tables/insert_table_3x3` | 52 µs | ±20% |

A regression in any of these beyond tolerance is a red flag for
Phase 1 or Phase 2 PRs.

## Acceptance thresholds (referenced by Phase 1 and Phase 2 exit criteria)

- **Phase 1 (InlineElement removal)**:
  - Memory floor (E + A) should drop by ≥30%.
  - Scenario **J** should drop by ≥70% (the InlineElement-clone bottleneck is gone).
  - No bench in the "regression watch zone" should slow down beyond tolerance.
- **Phase 2 (rope swap)**:
  - `xl/insert_char_at_end_of_1mb` should drop by ≥10×.
  - `xl/find_all_100_matches_in_1mb` should drop by ≥3×.
  - `plain_text_io/set_plain_text/large/1000para` should drop by ≥10×.
  - Memory floor (E) should drop by ≥50% (to ~30–40 KiB).
  - Marginal cost on F should drop to ≤4 B/char (today ~2 B/char marginal but with high constant).
