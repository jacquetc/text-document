use text_document::format::{CharFormat, ImageFormat};

#[test]
fn char_format() {
    let format = CharFormat::new();
    //assert_eq!(result, 4);
}

#[test]
fn image_format() {
    let mut format = ImageFormat::new();
    format.height = Some(40);

    assert_eq!(format.height, Some(40));
}
