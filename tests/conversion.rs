#![cfg(test)]
mod common;
use common::setup_text_document;
use text_document::text_document_reader::TextDocumentReader;
use text_document::text_document_writer::TextDocumentWriter;

#[test]
fn set_plain_text() {
    let mut text_document = setup_text_document();

    text_document.set_plain_text("text").unwrap();
    assert_eq!(text_document.get_plain_text(), "text");

    text_document.set_plain_text("").unwrap();
    assert_eq!(text_document.get_plain_text(), "");

    text_document.set_plain_text("line1\nline2").unwrap();
    assert_eq!(text_document.get_plain_text(), "line1\nline2");

    text_document.set_plain_text("line1\nline2\n").unwrap();
    assert_eq!(text_document.get_plain_text(), "line1\nline2\n");
}

#[test]
fn read_plain_text() {
    let mut text_document = setup_text_document();

    TextDocumentReader::new(&mut text_document)
        .read_plain_text_file("tests/data/empty.txt")
        .unwrap();
    assert_eq!(text_document.get_plain_text(), "");

    TextDocumentReader::new(&mut text_document)
        .read_plain_text_file("tests/data/one_line.txt")
        .unwrap();
    assert_eq!(text_document.get_plain_text(), "line1");

    TextDocumentReader::new(&mut text_document)
        .read_plain_text_file("tests/data/two_lines.txt")
        .unwrap();
    assert_eq!(text_document.get_plain_text(), "line1\nline2");
}

#[test]
fn write_plain_text() {
    let mut text_document = setup_text_document();

    text_document.set_plain_text("line1\nline2").unwrap();

    // temporary file
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let temp_file_path = temp_file.path().to_str().unwrap();

    TextDocumentWriter::new(&text_document)
        .write_plain_text_file(temp_file_path)
        .unwrap();

    let content = std::fs::read_to_string(temp_file_path).unwrap();
    assert_eq!(content, "line1\nline2");
}

#[test]
fn set_markdown_text() {
    let mut text_document = setup_text_document();

    text_document.set_markdown("# title\n\nparagraph").unwrap();
    assert_eq!(text_document.get_markdown_text(), "# title\n\nparagraph");

    text_document.set_markdown("").unwrap();
    assert_eq!(text_document.get_markdown_text(), "");

    text_document
        .set_markdown("# title\n\n_**p**ar_ag<u>ra</u>~~ph~~")
        .unwrap();
    assert_eq!(
        text_document.get_markdown_text(),
        "# title\n\n_**p**ar_ag<u>ra</u>~~ph~~"
    );

    // a lone newline character is not a paragraph
    text_document
        .set_markdown("# title\n\nparagraph\n")
        .unwrap();
    assert_eq!(text_document.get_markdown_text(), "# title\n\nparagraph");
}
