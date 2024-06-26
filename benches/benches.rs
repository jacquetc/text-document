use criterion::{black_box, criterion_group, criterion_main, Criterion};
use text_document::TextDocument;

fn bench_set_markdown_conversion_million_word(c: &mut Criterion) {
    let mut text_document = TextDocument::new();

    let mut text = String::new();
    for _ in 0..100_000 {
        text.push_str(
            "**Lorem** ipsum dolor sit amet, _consectet<u>ur</u>_ adip~~iscing elit, sed~~ do",
        );
    }

    c.bench_function("set_markdown", |b| {
        b.iter(|| text_document.set_markdown(black_box(text.clone())))
    });
}

fn bench_get_markdown_conversion_million_word(c: &mut Criterion) {
    let mut text_document = TextDocument::new();

    let mut text = String::new();
    for _ in 0..100_000 {
        text.push_str(
            "**Lorem** ipsum dolor sit amet, _consectet<u>ur</u>_ adip~~iscing elit, sed~~ do",
        );
    }
    text_document.set_markdown(text);

    c.bench_function("get_markdown", |b| {
        b.iter(|| text_document.get_markdown_text())
    });
}

criterion_group!(
    benches,
    bench_set_markdown_conversion_million_word,
    bench_get_markdown_conversion_million_word
);
criterion_main!(benches);
