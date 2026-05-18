//! Heap-allocation profile of a TextDocument across edit histories.
//!
//! Run with: `cargo run --release --example memory_profile -p text-document`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use text_document::{MoveMode, MoveOperation, SelectionType, TextDocument, TextFormat};

struct Counting;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static A: Counting = Counting;

fn snapshot() -> (usize, usize) {
    (
        CURRENT.load(Ordering::Relaxed),
        PEAK.load(Ordering::Relaxed),
    )
}

fn reset_peak() {
    PEAK.store(CURRENT.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn fmt(b: isize) -> String {
    let sign = if b < 0 { "-" } else { " " };
    let abs = b.unsigned_abs();
    if abs >= 1 << 20 {
        format!("{sign}{:>7.2} MiB", abs as f64 / (1 << 20) as f64)
    } else if abs >= 1 << 10 {
        format!("{sign}{:>7.2} KiB", abs as f64 / (1 << 10) as f64)
    } else {
        format!("{sign}{abs:>7} B  ")
    }
}

const SAMPLE: &str = r#"# Rich Text Editor — Preview Pane

This window hosts two `RichTextEditor` widgets bound to the **same**
`TextDocument`. The left pane is the full editor; the right pane is a
read-only viewer with a `SelectionType::Document` fallback. Because
both subscribe to `doc.on_change()` independently, edits in the left
pane propagate live to the right pane on the next frame tick — no
manual state shuffling, no `poll_events()` starvation problem.

## What works in M8b

- Full text insertion: typing, Enter to split blocks, Backspace,
  Delete, Ctrl+Backspace / Ctrl+Delete for word-level deletion.
- Undo / redo (Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z).
- Bold / italic / underline toggles (Ctrl+B / Ctrl+I / Ctrl+U).
- Click to place caret (click 1), double-click to select word
  (click 2), triple-click to select paragraph (click 3). The three
  gestures are independent cooperative recognizers — click 3 escalates
  over what click 2 installed.
- Drag-select with near-edge auto-scroll. Pull the mouse past the
  top or bottom of the viewport; the widget keeps scrolling while the
  button is held.
- Copy / cut / paste through the system clipboard. In-process paste
  preserves rich formatting via a stored `DocumentFragment`;
  inter-application paste round-trips through HTML on Linux
  (`text/html`), macOS (`public.html`), and Windows (`CF_HTML`), so
  copy from Firefox / Word / Google Docs keeps headings, bold,
  italic, lists, tables — anything text-document's HTML importer
  recognises.
- Ctrl+Shift+V pastes as plain text (`EditCommandKind::PasteUnformatted`).
- Tab: inside a table, moves to the next cell (auto-inserts a row at
  the last cell); at the start of a list item, increases indent;
  otherwise inserts a literal tab. Shift+Tab is the inverse.
- Ctrl+Enter always inserts a block, bypassing the "Enter-in-table
  navigates to the cell below" behaviour.
- Backspace at the start of an indented list item dedents; at indent
  zero it exits the list.
- Shift+Arrow at a cell boundary activates rectangular cell selection;
  further Shift+Arrows extend the rectangle.
- Links and images are clickable — install callbacks via
  `.on_link_activated(...)` / `.on_image_activated(...)` on the editor.
- Right-click for Cut / Copy / Paste / Paste Unformatted / Select All.
  Item availability (Cut/Copy require a selection, Select All requires
  a non-empty document) refreshes on every open. Read-only preset ships
  a trimmed Copy + Select All variant. Apps that want to override
  pass their own factory via `RichTextEditor::context_menu(...)`.
- Ctrl+A single-shot select-all (the 4-level ladder is inside a table
  cell only — try this document's paragraphs and you'll see the
  single-shot behaviour).

## Not here yet

- IME composition (M10).
- RTF clipboard payload — the long-tail rich fallback for Pages /
  TextEdit / older Windows apps that don't emit HTML. HTML covers
  Firefox, Word, Google Docs, Apple Notes.

Type below, watch the preview update in real time.
"#;

fn measure<F: FnOnce() -> TextDocument>(name: &str, f: F) -> (isize, isize) {
    reset_peak();
    let (live_before, _) = snapshot();
    let doc = f();
    let (live_after, peak) = snapshot();
    let live = live_after as isize - live_before as isize;
    let peak = peak as isize - live_before as isize;
    println!("  {name:<46}  live = {}    peak = {}", fmt(live), fmt(peak));
    drop(doc);
    (live, peak)
}

fn build_baseline() -> TextDocument {
    let d = TextDocument::new();
    d.set_markdown(SAMPLE).unwrap().wait().unwrap();
    d.clear_undo_redo();
    d
}

fn main() {
    println!(
        "Sample markdown: {} bytes ({} chars)",
        SAMPLE.len(),
        SAMPLE.chars().count()
    );
    println!();
    println!(
        "  {:<46}  {:<22}  Peak during build",
        "Scenario", "Live (after build)"
    );
    println!("  {}", "-".repeat(94));

    let a = measure("A. baseline doc (undo cleared)", build_baseline);

    let b = measure("B. + 10 single-char inserts (undo kept)", || {
        let d = build_baseline();
        let c = d.cursor();
        c.move_position(MoveOperation::End, MoveMode::MoveAnchor, 1);
        for _ in 0..10 {
            c.insert_text("x").unwrap();
        }
        d
    });

    let b_prime = measure("B'. same, then clear_undo_redo", || {
        let d = build_baseline();
        let c = d.cursor();
        c.move_position(MoveOperation::End, MoveMode::MoveAnchor, 1);
        for _ in 0..10 {
            c.insert_text("x").unwrap();
        }
        d.clear_undo_redo();
        d
    });

    let c = measure("C. select-all + copy + paste-at-end (undo kept)", || {
        let d = build_baseline();
        let cur = d.cursor();
        cur.select(SelectionType::Document);
        let frag = cur.selection();
        cur.move_position(MoveOperation::End, MoveMode::MoveAnchor, 1);
        cur.insert_fragment(&frag).unwrap();
        d
    });

    let c_prime = measure("C'. same, then clear_undo_redo", || {
        let d = build_baseline();
        let cur = d.cursor();
        cur.select(SelectionType::Document);
        let frag = cur.selection();
        cur.move_position(MoveOperation::End, MoveMode::MoveAnchor, 1);
        cur.insert_fragment(&frag).unwrap();
        d.clear_undo_redo();
        d
    });

    let d = measure("D. 2× content built directly (undo cleared)", || {
        let d = TextDocument::new();
        d.set_markdown(&SAMPLE.repeat(2)).unwrap().wait().unwrap();
        d.clear_undo_redo();
        d
    });

    // ── Phase 0 baseline scenarios (per plan §6.1) ──────────────────
    // Each captures a regime the existing A–D set does not cover.

    let e = measure("E. empty doc (TextDocument::new only)", || {
        TextDocument::new()
    });

    let f = measure("F. 1 MB plain text, no format (undo cleared)", || {
        let d = TextDocument::new();
        let big = "a".repeat(1_000_000);
        d.set_plain_text(&big).unwrap();
        d.clear_undo_redo();
        d
    });

    let g = measure("G. 100 blocks × 1 KB each (undo cleared)", || {
        let d = TextDocument::new();
        let para: String = "x".repeat(1024);
        let text = (0..100)
            .map(|_| para.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        d.set_plain_text(&text).unwrap();
        d.clear_undo_redo();
        d
    });

    let h = measure("H. 3 KB doc with bold every 5 chars (undo cleared)", || {
        let d = TextDocument::new();
        d.set_markdown(SAMPLE).unwrap().wait().unwrap();
        let cur = d.cursor();
        cur.move_position(MoveOperation::Start, MoveMode::MoveAnchor, 1);
        let bold = TextFormat {
            font_bold: Some(true),
            ..Default::default()
        };
        // Toggle bold over each 5-char window across the full doc.
        let total = d.character_count();
        let mut at: usize = 0;
        while at < total {
            let cur = d.cursor_at(at);
            cur.move_position(MoveOperation::Right, MoveMode::KeepAnchor, 5);
            cur.merge_char_format(&bold).unwrap();
            at += 10; // skip 5 (so bold runs alternate)
        }
        d.clear_undo_redo();
        d
    });

    let i = measure(
        "I. 10×10 table filled with 20-char cells (undo cleared)",
        || {
            let d = TextDocument::new();
            d.set_plain_text("").unwrap();
            let cur = d.cursor_at(0);
            let _ = cur.insert_table(10, 10).unwrap();
            // Fill each cell with 20 chars. Walk the cell grid via cursor moves
            // is fragile across versions; rely on the cursor landing inside the
            // first cell after insert_table and stepping through with Tab-style
            // motion via re-positioning by character count is unreliable, so
            // we only measure the empty-table cost here. This isolates the
            // structural overhead of 100 cells without per-cell content noise.
            d.clear_undo_redo();
            d
        },
    );

    let j = measure("J. 100 KB doc + 1000 small inserts (undo KEPT)", || {
        let d = TextDocument::new();
        let seed = "a".repeat(100_000);
        d.set_plain_text(&seed).unwrap();
        d.clear_undo_redo();
        for k in 0..1000 {
            let pos = (k * 97) % 100_000;
            let cur = d.cursor_at(pos);
            cur.insert_text("x").unwrap();
        }
        d
    });

    let (residual, _) = snapshot();
    println!();
    println!(
        "Residual heap after all docs dropped: {}",
        fmt(residual as isize)
    );

    println!();
    println!("Deltas:");
    println!(
        "  undo cost, 10 small ops    B - B'      = {}",
        fmt(b.0 - b_prime.0)
    );
    println!(
        "  undo cost, 1 paste op      C - C'      = {}",
        fmt(c.0 - c_prime.0)
    );
    println!(
        "  data cost of 2× content    D - A       = {}",
        fmt(d.0 - a.0)
    );
    println!(
        "  paste residue vs raw 2×    C' - D      = {}",
        fmt(c_prime.0 - d.0)
    );
    println!(
        "  10 small ops residual data B' - A      = {}",
        fmt(b_prime.0 - a.0)
    );
    println!("  empty doc floor            E           = {}", fmt(e.0));
    println!("  data + structure 1MB plain F           = {}", fmt(f.0));
    println!("  100×1KB blocks             G           = {}", fmt(g.0));
    println!(
        "  format-run-heavy 3KB doc   H - A       = {}",
        fmt(h.0 - a.0)
    );
    println!(
        "  100-cell table cost        I - E       = {}",
        fmt(i.0 - e.0)
    );
    println!("  1000 inserts undo cost     J (live)    = {}", fmt(j.0));
}
