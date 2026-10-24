use std::path::Path;

use objc2::AnyThread as _;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSPasteboard, NSPasteboardItem,
    NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};

const PNG_UTI: &str = "public.png";

/// An image to copy: its encoded bytes, their uniform type identifier
/// (`public.png`, `public.jpeg`, `com.compuserve.gif`,
/// `org.webmproject.webp`) and a file on disk with the same bytes.
pub struct ClipboardImage<'a> {
    pub bytes: &'a [u8],
    pub uti: &'a str,
    pub file: &'a Path,
}

/// Copies an image as one pasteboard item with several representations, so
/// each app takes the one it understands: the file URL, the bytes under
/// their own type, and PNG and TIFF renderings. Returns the types written.
pub fn copy_image_to_clipboard(image: ClipboardImage<'_>) -> Result<Vec<String>, String> {
    write_image(image, &NSPasteboard::generalPasteboard())
}

fn write_image(
    image: ClipboardImage<'_>,
    pasteboard: &NSPasteboard,
) -> Result<Vec<String>, String> {
    let path = image
        .file
        .to_str()
        .ok_or("The image file path is not UTF-8")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path))
        .absoluteString()
        .ok_or("The image file path makes no URL")?;
    let bytes = NSData::with_bytes(image.bytes);
    let rendered = NSImage::initWithData(NSImage::alloc(), &bytes)
        .ok_or_else(|| format!("AppKit can't decode this {} image", image.uti))?;

    let item = NSPasteboardItem::new();
    let mut written = Vec::new();
    // SAFETY: the pasteboard type constants are framework statics.
    unsafe {
        if item.setString_forType(&url, NSPasteboardTypeFileURL) {
            written.push(NSPasteboardTypeFileURL.to_string());
        }
        if item.setData_forType(&bytes, &NSString::from_str(image.uti)) {
            written.push(image.uti.to_string());
        }
        if let Some(tiff) = rendered.TIFFRepresentation() {
            if image.uti != PNG_UTI
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
        return Err("The pasteboard refused the image".into());
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

    /// On a private pasteboard, so the user's clipboard is left alone.
    fn write_sample(name: &str, uti: &str, bytes: &[u8]) -> (Retained<NSPasteboard>, Vec<String>) {
        let file = std::env::temp_dir().join(format!("quick-search-test-{name}"));
        std::fs::write(&file, bytes).unwrap();
        let pasteboard = NSPasteboard::pasteboardWithUniqueName();
        let image = ClipboardImage {
            bytes,
            uti,
            file: &file,
        };
        let written = write_image(image, &pasteboard).unwrap();
        (pasteboard, written)
    }

    #[test]
    fn a_gif_gets_file_url_own_bytes_png_and_tiff() {
        let bytes = include_bytes!("../../../src/public/quick-search/images/sample-animated.gif");
        let (pasteboard, written) = write_sample("a.gif", "com.compuserve.gif", bytes);

        assert_eq!(
            written,
            [
                "public.file-url",
                "com.compuserve.gif",
                "public.png",
                "public.tiff"
            ]
        );
        // SAFETY: framework statics.
        unsafe {
            let png = pasteboard
                .dataForType(NSPasteboardTypePNG)
                .unwrap()
                .to_vec();
            assert_eq!(&png[..4], b"\x89PNG");
            let url = pasteboard.stringForType(NSPasteboardTypeFileURL).unwrap();
            assert!(url.to_string().starts_with("file:///"), "{url}");
        }
        let gif = pasteboard
            .dataForType(&NSString::from_str("com.compuserve.gif"))
            .unwrap()
            .to_vec();
        assert_eq!(gif, bytes);
    }

    #[test]
    fn a_webp_is_decoded_for_the_png_and_tiff_renderings() {
        let bytes = include_bytes!("../../../src/public/quick-search/images/sample.webp");
        let (_pasteboard, written) = write_sample("a.webp", "org.webmproject.webp", bytes);
        assert_eq!(
            written,
            [
                "public.file-url",
                "org.webmproject.webp",
                "public.png",
                "public.tiff"
            ]
        );
    }

    #[test]
    fn a_png_is_not_rendered_to_png_again() {
        let bytes = include_bytes!("../../../src/public/quick-search/images/sample.png");
        let (_pasteboard, written) = write_sample("a.png", "public.png", bytes);
        assert_eq!(written, ["public.file-url", "public.png", "public.tiff"]);
    }

    #[test]
    fn bytes_that_are_no_image_are_refused() {
        let file = std::env::temp_dir().join("quick-search-test-not-an-image");
        std::fs::write(&file, b"hello").unwrap();
        let image = ClipboardImage {
            bytes: b"hello",
            uti: "public.png",
            file: &file,
        };
        let error = write_image(image, &NSPasteboard::pasteboardWithUniqueName()).unwrap_err();
        assert!(error.contains("can't decode"), "{error}");
    }
}
