//! The macOS clipboard: one pasteboard item with the image's file, its
//! own bytes, and PNG and TIFF renderings.

use objc2::AnyThread as _;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSPasteboard, NSPasteboardItem,
    NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};

use super::{ClipboardImage, PNG_TYPE};

/// Copies an image as one pasteboard item with several representations:
/// the file URL, the bytes under their own type, and PNG and TIFF
/// renderings. Returns the types written.
pub(super) fn copy_image(image: ClipboardImage<'_>) -> Result<Vec<String>, String> {
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
    use crate::clipboard::tests::GIF;

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
    fn clipboard_macos_copy_image_writes_a_gif_as_its_file_its_bytes_png_and_tiff() {
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
    fn clipboard_macos_copy_image_refuses_bytes_that_are_no_image() {
        let error = written("public.png", b"hello").expect_err("not an image");
        assert!(error.contains("cannot be read"), "{error}");
    }
}
