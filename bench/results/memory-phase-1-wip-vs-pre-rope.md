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
- Phase 1.13c (use case readers): NOT STARTED — requires plumbing
  (a `store()` method on CommandUnitOfWork + bulk-impl in ~32 UoW
  files) before each of 8 use cases can be migrated.
- Phase 1.13d (DocumentFragment internal): NOT STARTED — affects
  clipboard interchange; FragmentElement schema must be preserved.
- Phase 1.14 (deletion): NOT STARTED — this is where the memory
  wins finally materialize. Requires Phase 1.13c first.
- Phase 1.17 (bench compare): N/A until 1.14 done.

The current state is a stable WIP checkpoint that Phase 2 (rope swap)
could pick up from directly. Phase 2 naturally subsumes 1.14 because
the rope replaces inline_elements entirely.
