//! Copying an image to the system clipboard on Apple's platforms, where
//! Tauri has nothing that keeps an animated GIF: each app pastes the form
//! it understands, the GIF's own bytes where GIFs are taken and a PNG
//! elsewhere. AppKit on macOS (`macos`), UIKit on iOS (`ios`). Text too
//! on macOS, for the quick picker.

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "macos")]
mod macos;

use std::path::Path;

const PNG_TYPE: &str = "public.png";

/// An image to copy: its encoded bytes, their uniform type identifier,
/// see `uniform_type_of`, and the file on this device with those bytes.
pub struct ClipboardImage<'a> {
    pub bytes: &'a [u8],
    pub uniform_type: &'a str,
    #[cfg_attr(
        target_os = "ios",
        expect(dead_code, reason = "no other iOS app reads the app's files")
    )]
    pub file: &'a Path,
}

/// The uniform type identifier Apple's platforms know an image content
/// type by, for those the clipboard takes.
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

/// Copies the image to the clipboard; call on the main thread, where
/// the platform's pasteboard and images live. Returns the types written.
pub fn copy_image(image: ClipboardImage<'_>) -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    return macos::copy_image(image);
    #[cfg(target_os = "ios")]
    return ios::copy_image(image);
}

/// Copies the text to the clipboard; call on the main thread.
#[cfg(target_os = "macos")]
pub fn copy_text(text: &str) -> Result<(), String> {
    macos::copy_text(text)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A GIF of one white pixel.
    pub(crate) const GIF: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
    ];

    #[test]
    fn clipboard_uniform_type_of_knows_the_images_it_copies() {
        assert_eq!(uniform_type_of("image/gif"), Some("com.compuserve.gif"));
        assert_eq!(uniform_type_of("image/png; charset=binary"), Some(PNG_TYPE));
        assert_eq!(uniform_type_of("image/svg+xml"), None);
    }
}
