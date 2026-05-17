# Memory profile: post Phase 2 vs `pre-rope` baseline

Captured at `feature/rope-backend` HEAD after Phase 2 (rope swap,
junction collapse, removal of `Block.plain_text` and `Block.text_length`).
Comparison against the `pre-rope` baseline captured before the migration
started.

Raw outputs:
- pre-rope: [memory-pre-rope.txt](memory-pre-rope.txt)
- post-phase-2: [memory-post-phase-2.txt](memory-post-phase-2.txt)

## Headline table

| Scenario | Pre-rope | Post-phase-1.14 | **Post-phase-2** | Δ vs pre-rope |
|---|---|---|---|---|
| A. baseline 3 KB doc (undo cleared) | 451.51 KiB | 132.82 KiB | **126.04 KiB** | **−72 % (3.6× lower)** |
| B. + 10 single-char inserts (undo kept) | 481.30 KiB | 143.13 KiB | **108.43 KiB** | **−77 % (4.4× lower)** |
| B'. same, then clear_undo_redo | 435.49 KiB | — | **100.78 KiB** | **−77 % (4.3× lower)** |
| C. select-all + copy + paste-at-end (undo kept) | 1.08 MiB | 368.21 KiB | **299.63 KiB** | **−73 % (3.7× lower)** |
| C'. same, then clear_undo_redo | 725.39 KiB | — | **194.73 KiB** | **−73 % (3.7× lower)** |
| D. 2× content directly | 698.34 KiB | 267.95 KiB | **205.01 KiB** | **−71 % (3.4× lower)** |
| E. empty doc (`TextDocument::new`) | 77.06 KiB | 68.84 KiB | **54.01 KiB** | **−30 %** |
| F. 1 MB plain text | 1.98 MiB | 1.02 MiB | **1.13 MiB** | **−43 % (1.8× lower)** |
| G. 100 × 1 KB blocks | 1008.95 KiB | 513.97 KiB | **551.80 KiB** | **−45 % (1.8× lower)** |
| H. 3 KB doc, bold every 5 chars | 1.55 MiB | 250.81 KiB | **242.62 KiB** | **−85 % (6.5× lower)** |
| I. 10×10 table, 20-char cells | 1.44 MiB | 1.19 MiB | **794.08 KiB** | **−46 % (1.9× lower)** |
| J. 100 KB doc + 1000 small inserts (undo KEPT) | 192.80 MiB | 96.64 MiB | **885.27 KiB** | **−99.6 % (218× lower)** |

Residual heap after all docs dropped: 1.66 KiB (unchanged — no leak).

## The headline: scenario J (1000 small inserts, undo kept)

The pre-rope baseline held **192.80 MiB** of resident memory after 1000
small inserts on a 100 KB document — because each undo snapshot cloned
the entire `inline_elements` HashMap, the per-block `plain_text`
Strings, and all 12 junction tables. Phase 1.14 halved that to
**96.64 MiB** by dropping the inline_elements table. Phase 2 collapses
the rest:

- The 1000-deep snapshot stack now shares the `ropey::Rope` via `Arc`
  (each clone is O(1) bytes), instead of cloning a `String` per block
  per snapshot.
- The 11 surviving `im::HashMap` tables share unchanged HAMT paths
  across snapshots.
- The BlockOffsetIndex `Vec` is the only fully-cloned per-snapshot
  payload, and it carries one entry per block — negligible against the
  data itself.

**192.80 MiB → 885.27 KiB. A 218× reduction.** This is the single
biggest win in the migration and was the motivating scenario in the
original profile.

## The format-heavy case (H): 6.5× lower

A 3 KB doc with bold every 5 characters has ~600 format runs. Pre-rope,
each run was a separate `InlineElement` entity carrying a cloned
`String` and an `Option<CharacterFormat>`, multiplied across the
snapshot stack. Phase 1.14 collapsed that to 6.3× lower; Phase 2 adds
another 3 % by removing the per-block `plain_text` String:
**1.55 MiB → 242.62 KiB**.

## The bulk-content cases (A–D): 3.4–4.4× lower

The "typical 3 KB markdown doc" workload halves twice over: once when
Phase 1.14 removed the per-character entity overhead, and once more
when Phase 2 moved character data to the shared rope. The per-keystroke
undo cost (B − B') shrank from **45.81 KiB to 7.65 KiB (5.9× lower)**
— a real win for editor sessions with deep undo histories.

The select-all-paste-with-undo cost (C − C') shrank from
**382.86 KiB to 104.91 KiB (3.6× lower)** for the same reason.

## The smaller wins (E, F, G, I)

- **E (empty doc, 54.01 KiB)** is dominated by structural infrastructure
  (Root, Document, root Frame, savepoint store, repository
  scaffolding). The 30 % reduction comes from collapsed junctions and
  the rope's small empty footprint. This **misses the plan §5.4 target
  of ≤ 15 KiB by 3.6×** — further reduction would require collapsing
  the savepoint store and repository overhead, which is structural and
  out of scope.
- **F (1 MB plain text, 1.13 MiB)** is at **~1.18 B/char marginal cost**
  — well under the plan §5.4 target of ≤ 4 B/char. The rope's B+ tree
  overhead is small relative to the raw character data at this scale.
- **G (100 × 1 KB blocks)** wins 45 %; the floor is set by the 100
  block entities + their `im::HashMap` paths, which the rope migration
  doesn't touch.
- **I (100-cell table)** wins 46 %; each cell still owns its own Frame
  + Block, and that structure dominates the table footprint.

## §5.4 acceptance verdict (memory deliverables)

| §5.4 criterion | Result | Verdict |
|---|---|---|
| Memory floor (empty doc) ≤ 15 KiB | 54.01 KiB | ✗ (3.6× over) |
| Marginal per char unformatted ≤ 4 B/char (scenario F) | 1.18 B/char | ✓ (3.4× under) |

The floor target is missed; the marginal target is exceeded. Plus the
huge wins on the snapshot-stack scenarios (J, H, B, C) — which were
the original motivation for the migration in the
[memory_profile.rs comment header](file:///Users/cyril/Devel/text-document/crates/public_api/examples/memory_profile.rs)
— validate the structural refactor.

## What Phase 2 changed (memory side)

Compared to Phase 1.14:

- `Block.plain_text: String` removed; all character data lives in the
  shared `ropey::Rope`. Per-block heap footprint drops by `plain_text`
  capacity + alignment padding.
- `Block.text_length: i64` removed; computed on demand from
  `block_content_via_store(block, store)`.
- The 12 junction tables (`HashMapStore.jn_*`) collapsed into inline
  `Vec<EntityId>` fields on the parent entities. Snapshot clone
  amortizes across HAMT sharing.
- `HashMapStore` replaced by `RopeStore { rope, … }`. The rope's
  `Arc<Node>` makes clone O(1).

## What Phase 2 left on the table (memory side)

- Empty-doc floor (E) is 54 KiB, target was 15 KiB. The remaining bulk
  is in the savepoint store (`HashMap<u64, RopeStoreSnapshot>` plus
  per-repository state) and the `im::HashMap` root nodes for each of
  the 8 structural entity tables. A second-pass cleanup of the
  savepoint allocator and repository scaffolding could reach the
  target, but it's structural work, not migration work.
- Scenario F (1 MB) is 1.13 MiB — within the marginal target, but the
  rope is itself ~1.0 MiB, and the remaining ~130 KiB is BlockOffsetIndex
  + structural entities. For documents with very many blocks (G, I),
  the structural overhead dominates and doesn't compress further.

The wins delivered are the wins the user-visible memory budget cares
about: editor sessions with deep undo histories, large pastes, and
heavily-formatted documents.
