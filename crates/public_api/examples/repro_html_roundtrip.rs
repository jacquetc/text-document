use text_document::{MoveMode, MoveOperation, TextDocument};

fn main() {
    let seed = "é/é🌍!mk\"c/7\t&.?'.\t?éE<\n\tv>\na\n]2]98Od🌍à!]àH0=yW/]'<&F( ]v]T\téà?2";
    eprintln!("seed: {:?}", seed);
    let doc1 = TextDocument::new();
    let op1 = doc1.set_html(seed).expect("set_html ok");
    op1.wait().expect("wait ok");
    eprintln!(
        "doc1 plain: {:?}\n  cc={} bc={}",
        doc1.to_plain_text(),
        doc1.character_count(),
        doc1.block_count()
    );
    let c = doc1.cursor_at(0);
    c.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    let html1 = c.selection().to_html();
    eprintln!("html1: {:?}", html1);
    eprintln!("---");
    let doc2 = TextDocument::new();
    let op2 = doc2.set_html(&html1).expect("set_html2 ok");
    op2.wait().expect("wait2 ok");
    eprintln!(
        "doc2 plain: {:?}\n  cc={} bc={}",
        doc2.to_plain_text(),
        doc2.character_count(),
        doc2.block_count()
    );
    let c2 = doc2.cursor_at(0);
    c2.move_position(MoveOperation::End, MoveMode::KeepAnchor, 1);
    let html2 = c2.selection().to_html();
    eprintln!("html2: {:?}", html2);
    eprintln!("---");
    if html1 == html2 {
        eprintln!("OK: idempotent");
    } else {
        eprintln!("MISMATCH: not idempotent");
    }
}
