//! Copying an image to the macOS clipboard so each app pastes the form
//! it understands: an animated GIF stays animated where GIFs are taken,
//! and is a still picture elsewhere.

use std::path::Path;

use objc2::AnyThread as _;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSPasteboard, NSPasteboardItem,
    NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};

const PNG_TYPE: &str = "public.png";

/// An image to copy: its encoded bytes, their uniform type identifier,
/// see `uniform_type_of`, and a file on disk with the same bytes.
pub struct ClipboardImage<'a> {
    pub bytes: &'a [u8],
    pub uniform_type: &'a str,
    pub file: &'a Path,
}

/// The uniform type identifier macOS knows an image content type by,
/// for those the clipboard takes.
pub fn uniform_type_of(content_type: &str) -> Option<&'static str> {
    match content_type.split(';').next().unwrap_or_default().trim() {
        "image/png" => Some(PNG_TYPE),
        "image/jpeg" => Some("public.jpeg"),
        "image/gif" => Some("com.compuserve.gif"),
        "image/webp" => Some("org.webmproject.webp"),
        "image/tiff" => Some("public.tiff"),
        "image/bmp" => Some("com.microsoft.bmp"),
        "image/heic" => Some("public.heic"),
        _ => None,
    }
}

/// Copies an image as one pasteboard item with several representations:
/// the file URL, the bytes under their own type, and PNG and TIFF
/// renderings. Returns the types written.
pub fn copy_image(image: ClipboardImage<'_>) -> Result<Vec<String>, String> {
    write_image(image, &NSPasteboard::generalPasteboard())
}

fn write_image(
    image: ClipboardImage<'_>,
    pasteboard: &NSPasteboard,
) -> Result<Vec<String>, String> {
    let path = image.file.to_str().ok_or("the image's path is not UTF-8")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path))
        .absoluteString()
        .ok_or("the image's path makes no URL")?;
    let bytes = NSData::with_bytes(image.bytes);
    let rendered = NSImage::initWithData(NSImage::alloc(), &bytes)
        .ok_or("this image cannot be read to copy it")?;

    let item = NSPasteboardItem::new();
    let mut written = Vec::new();
    // SAFETY: the pasteboard type constants are framework statics.
    unsafe {
        if item.setString_forType(&url, NSPasteboardTypeFileURL) {
            written.push(NSPasteboardTypeFileURL.to_string());
        }
        if item.setData_forType(&bytes, &NSString::from_str(image.uniform_type)) {
            written.push(image.uniform_type.to_string());
        }
        if let Some(tiff) = rendered.TIFFRepresentation() {
            if image.uniform_type != PNG_TYPE
                && let Some(png) = render_png(&tiff)
                && item.setData_forType(&png, NSPasteboardTypePNG)
            {
                written.push(NSPasteboardTypePNG.to_string());
            }
            if item.setData_forType(&tiff, NSPasteboardTypeTIFF) {
                written.push(NSPasteboardTypeTIFF.to_string());
            }
        }
    }

    pasteboard.clearContents();
    let objects = NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]);
    if !pasteboard.writeObjects(&objects) {
        return Err("the clipboard refused the image".into());
    }
    Ok(written)
}

fn render_png(tiff: &NSData) -> Option<Retained<NSData>> {
    let bitmap = NSBitmapImageRep::imageRepWithData(tiff)?;
    // SAFETY: the properties dictionary is empty and outlives the call.
    unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GIF of one white pixel.
    const GIF: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
    ];

    /// On a pasteboard of the test's own, so the person's clipboard is
    /// left alone.
    fn written(
        uniform_type: &str,
        bytes: &[u8],
    ) -> Result<(Retained<NSPasteboard>, Vec<String>), String> {
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join("image");
        std::fs::write(&file, bytes).expect("a file");
        let pasteboard = NSPasteboard::pasteboardWithUniqueName();
        let image = ClipboardImage {
            bytes,
            uniform_type,
            file: &file,
        };
        let written = write_image(image, &pasteboard)?;
        Ok((pasteboard, written))
    }

    #[test]
    fn pasteboard_copy_image_writes_a_gif_as_its_file_its_bytes_png_and_tiff() {
        let (pasteboard, written) = written("com.compuserve.gif", GIF).expect("copied");
        assert_eq!(
            written,
            [
                "public.file-url",
                "com.compuserve.gif",
                "public.png",
                "public.tiff"
            ]
        );
        // SAFETY: a framework static.
        let png = unsafe { pasteboard.dataForType(NSPasteboardTypePNG) }
            .expect("a PNG")
            .to_vec();
        assert_eq!(&png[..4], b"\x89PNG");
        let gif = pasteboard
            .dataForType(&NSString::from_str("com.compuserve.gif"))
            .expect("the GIF")
            .to_vec();
        assert_eq!(gif, GIF, "the animation is kept");
    }

    #[test]
    fn pasteboard_copy_image_refuses_bytes_that_are_no_image() {
        let error = written("public.png", b"hello").expect_err("not an image");
        assert!(error.contains("cannot be read"), "{error}");
    }

    #[test]
    fn pasteboard_uniform_type_of_knows_the_images_it_copies() {
        assert_eq!(uniform_type_of("image/gif"), Some("com.compuserve.gif"));
        assert_eq!(uniform_type_of("image/svg+xml"), None);
    }
}
