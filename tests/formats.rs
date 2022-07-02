use text_document::format::{ImageFormat, TextFormat};

#[test]
fn text_format() {
    let mut format = TextFormat::new();

    assert!(!format.font.bold());
    format.font.set_bold(true);
    assert!(format.font.bold());

    assert!(!format.font.italic());
    format.font.set_italic(true);
    assert!(format.font.italic());
}

#[test]
fn image_format() {
    let mut format = ImageFormat::new();
    format.height = Some(40);

    assert_eq!(format.height, Some(40));
}
