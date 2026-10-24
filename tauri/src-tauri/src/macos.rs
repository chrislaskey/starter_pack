//! The macOS calls the quick search popup needs that Tauri has no API for:
//! which app is in front, where its text caret is (Accessibility API),
//! whether the user granted Accessibility access, putting an image on the
//! clipboard in several representations (`NSPasteboard`), and pressing ⌘V
//! in another app (Core Graphics events). These are the same calls a Swift app makes,
//! through the `objc2-app-kit`, `core-foundation` and `core-graphics`
//! bindings, so no Swift toolchain is needed.

use std::ffi::c_void;
use std::path::Path;

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc2::AnyThread as _;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSPasteboard,
    NSPasteboardItem, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
    NSRunningApplication, NSWorkspace,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};
use serde::Serialize;

type AXUIElementRef = CFTypeRef;
type AXValueRef = CFTypeRef;
type AXError = i32;
type Boolean = u8;

const AX_ERROR_SUCCESS: AXError = 0;
/// `kAXValueTypeCGPoint`, `kAXValueTypeCGSize`, `kAXValueTypeCGRect`
const AX_VALUE_TYPE_CGPOINT: u32 = 1;
const AX_VALUE_TYPE_CGSIZE: u32 = 2;
const AX_VALUE_TYPE_CGRECT: u32 = 3;
/// The virtual key code of the V key on an ANSI keyboard.
const KEY_V: CGKeyCode = 9;

// The Accessibility API, part of ApplicationServices (HIServices), which has
// no objc2 crate: declared by hand.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXValueGetValue(value: AXValueRef, value_type: u32, out: *mut c_void) -> Boolean;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_seconds: f32) -> AXError;
}

/// How long one Accessibility call may wait for the other app. The default
/// is several seconds, and a busy or stuck app makes every call wait it
/// out.
const AX_CALL_TIMEOUT_SECONDS: f32 = 0.5;

/// The app's Accessibility element, with the short call timeout.
unsafe fn application_element(pid: i32) -> CFType {
    // SAFETY: the create rule applies; the timeout call only reads the
    // element.
    unsafe {
        let app = CFType::wrap_under_create_rule(AXUIElementCreateApplication(pid));
        AXUIElementSetMessagingTimeout(app.as_CFTypeRef(), AX_CALL_TIMEOUT_SECONDS);
        app
    }
}

/// Whether the user granted this app Accessibility access in System
/// Settings › Privacy & Security › Accessibility. With `prompt`, macOS shows
/// its dialog asking for it when it isn't granted yet.
///
/// Launched from a terminal (`cargo tauri dev`), the process this is checked
/// for is the terminal app, not this one: macOS attributes permissions to
/// the "responsible" app.
pub fn is_accessibility_trusted(prompt: bool) -> bool {
    // SAFETY: `kAXTrustedCheckOptionPrompt` is a constant the framework
    // owns, so wrapping it under the get rule only retains it, and the
    // dictionary outlives the call.
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::from(prompt))]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
    }
}

/// The app in front, the one the user was typing in.
#[derive(Clone, Debug, Serialize)]
pub struct FrontmostApp {
    pub pid: i32,
    pub name: String,
    pub bundle_id: Option<String>,
}

pub fn frontmost_app() -> Option<FrontmostApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    Some(FrontmostApp {
        pid: app.processIdentifier(),
        name: app
            .localizedName()
            .map(|name| name.to_string())
            .unwrap_or_default(),
        bundle_id: app.bundleIdentifier().map(|id| id.to_string()),
    })
}

/// Whether this app is the one in front. `NSApplication.isActive` isn't
/// used because it stays true after launch even while another app is in
/// front; the frontmost application is what the window server says.
pub fn is_this_app_in_front() -> bool {
    frontmost_app().is_some_and(|app| app.pid == std::process::id() as i32)
}

/// Brings another app to the front. Since macOS 14 the system may refuse
/// when the user is interacting with yet another app, and the result says
/// only whether the request was accepted.
pub fn activate_app(pid: i32) -> bool {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .is_some_and(|app| app.activateWithOptions(NSApplicationActivationOptions::empty()))
}

/// A rectangle in screen points, with the origin at the top left of the main
/// display, like Tauri's logical window positions.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Where an app accepts text: the caret itself when the app reports it,
/// else the whole focused text field.
#[derive(Clone, Copy, Debug)]
pub enum TextBounds {
    Caret(ScreenRect),
    TextField(ScreenRect),
}

/// The caret, or the focused text field, of the app with `pid`, through the
/// Accessibility API. Needs Accessibility access. The error says which step
/// failed and why, for the log and the popup.
///
/// Native text views, Safari and Firefox report the caret
/// (`AXBoundsForRange` of `AXSelectedTextRange`). Chromium and Electron
/// apps report nothing until accessibility is switched on in them, see
/// `enable_accessibility`. Terminals have no text caret to report.
pub fn focused_text_bounds(pid: i32) -> Result<TextBounds, String> {
    if !is_accessibility_trusted(false) {
        return Err("no Accessibility access".into());
    }

    // SAFETY: every ref the API returns follows the create rule, so wrapping
    // it in `CFType` releases it when dropped.
    unsafe {
        let app = application_element(pid);
        let focused = copy_attribute(&app, "AXFocusedUIElement")
            .map_err(|error| format!("no focused element: {error}"))?;

        let caret = copy_attribute(&focused, "AXSelectedTextRange")
            .and_then(|range| copy_parameterized_attribute(&focused, "AXBoundsForRange", &range))
            .and_then(|bounds| rect_value(&bounds));
        let caret_error = match caret {
            Ok(rect) if rect.x != 0.0 || rect.y != 0.0 || rect.height != 0.0 => {
                return Ok(TextBounds::Caret(rect));
            }
            Ok(_) => "empty rectangle".to_string(),
            Err(error) => error,
        };

        match element_bounds(&focused) {
            Ok(rect) => Ok(TextBounds::TextField(rect)),
            Err(error) => Err(format!("caret: {caret_error}; text field: {error}")),
        }
    }
}

/// Switches accessibility on in a Chromium or Electron app (Slack, Discord,
/// VS Code, Obsidian…). They build their accessibility tree only once
/// something asks for it, and `AXManualAccessibility` is the documented way
/// to ask (Electron's accessibility guide). Harmless for other apps, which
/// report the attribute as unsupported.
pub fn enable_accessibility(pid: i32) -> Result<(), String> {
    // SAFETY: as in `focused_text_bounds`; the boolean is a constant.
    unsafe {
        let app = application_element(pid);
        let attribute = CFString::new("AXManualAccessibility");
        let error = AXUIElementSetAttributeValue(
            app.as_CFTypeRef(),
            attribute.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
        if error == AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(describe_error(error))
        }
    }
}

/// The element's frame: `AXFrame` where supported, else position and size.
unsafe fn element_bounds(element: &CFType) -> Result<ScreenRect, String> {
    if let Ok(frame) = unsafe { copy_attribute(element, "AXFrame") }
        && let Ok(rect) = unsafe { rect_value(&frame) }
    {
        return Ok(rect);
    }
    let position = unsafe { copy_attribute(element, "AXPosition") }?;
    let size = unsafe { copy_attribute(element, "AXSize") }?;
    let mut point = CGPoint::new(0.0, 0.0);
    let mut extent = CGSize::new(0.0, 0.0);
    // SAFETY: the out pointers match the value types asked for.
    let got_point = unsafe {
        AXValueGetValue(
            position.as_CFTypeRef(),
            AX_VALUE_TYPE_CGPOINT,
            &mut point as *mut CGPoint as *mut c_void,
        )
    };
    let got_size = unsafe {
        AXValueGetValue(
            size.as_CFTypeRef(),
            AX_VALUE_TYPE_CGSIZE,
            &mut extent as *mut CGSize as *mut c_void,
        )
    };
    if got_point == 0 || got_size == 0 {
        return Err("position or size is not a point and size".into());
    }
    Ok(ScreenRect {
        x: point.x,
        y: point.y,
        width: extent.width,
        height: extent.height,
    })
}

unsafe fn rect_value(value: &CFType) -> Result<ScreenRect, String> {
    let mut rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0));
    // SAFETY: the out pointer is a `CGRect`, matching the type asked for.
    let got = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef(),
            AX_VALUE_TYPE_CGRECT,
            &mut rect as *mut CGRect as *mut c_void,
        )
    };
    if got == 0 {
        return Err("not a rectangle".into());
    }
    Ok(ScreenRect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    })
}

unsafe fn copy_attribute(element: &CFType, attribute: &str) -> Result<CFType, String> {
    let name = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: the caller upholds the create rule described in
    // `focused_text_bounds`.
    let error = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if error != AX_ERROR_SUCCESS || value.is_null() {
        return Err(format!("{attribute} {}", describe_error(error)));
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

unsafe fn copy_parameterized_attribute(
    element: &CFType,
    attribute: &str,
    parameter: &CFType,
) -> Result<CFType, String> {
    let name = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: as in `copy_attribute`.
    let error = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            parameter.as_CFTypeRef(),
            &mut value,
        )
    };
    if error != AX_ERROR_SUCCESS || value.is_null() {
        return Err(format!("{attribute} {}", describe_error(error)));
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

/// The `AXError` codes in words, see `AXError.h`.
fn describe_error(error: AXError) -> String {
    match error {
        0 => "returned nothing".into(),
        -25200 => "failed".into(),
        -25201 => "illegal argument".into(),
        -25202 => "invalid element (the app or element is gone)".into(),
        -25204 => "cannot complete (the app isn't answering)".into(),
        -25205 => "unsupported by this app".into(),
        -25208 => "not implemented by this app".into(),
        -25211 => "API disabled (no Accessibility access)".into(),
        -25212 => "has no value".into(),
        -25213 => "parameterized attribute unsupported by this app".into(),
        other => format!("AXError {other}"),
    }
}

/// An image to put on the clipboard: its encoded bytes, their uniform type
/// identifier (`public.png`, `public.jpeg`, `com.compuserve.gif`,
/// `org.webmproject.webp`) and a file on disk with the same bytes.
pub struct ImageForClipboard<'a> {
    pub bytes: &'a [u8],
    pub uti: &'a str,
    pub file: &'a Path,
}

/// Puts an image on the clipboard as one pasteboard item with several
/// representations, so each app takes the one it understands:
///
/// * the file URL, which apps that upload or attach files take, and which
///   keeps a GIF's animation and a WebP's format (AppKit serves the legacy
///   `NSFilenamesPboardType` from it to apps that still ask for that; an
///   item refuses to hold the legacy type itself)
/// * the bytes under their own type, for apps that read that format
/// * PNG and TIFF renderings, which nearly every app reads; for an
///   animated GIF these are its first frame
///
/// Neither Tauri nor the clipboard plugins write several representations
/// to one item, so this talks to `NSPasteboard` directly.
pub fn copy_image_to_clipboard(image: ImageForClipboard<'_>) -> Result<Vec<String>, String> {
    write_image(image, &NSPasteboard::generalPasteboard())
}

fn write_image(
    image: ImageForClipboard<'_>,
    pasteboard: &NSPasteboard,
) -> Result<Vec<String>, String> {
    let path = image
        .file
        .to_str()
        .ok_or("The image file path is not UTF-8")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    let url_string = url
        .absoluteString()
        .ok_or("The image file path makes no URL")?;
    let bytes = NSData::with_bytes(image.bytes);
    let own_type = NSString::from_str(image.uti);

    let item = NSPasteboardItem::new();
    let mut written = Vec::new();
    // SAFETY: the pasteboard type constants are framework statics.
    unsafe {
        if item.setString_forType(&url_string, NSPasteboardTypeFileURL) {
            written.push(NSPasteboardTypeFileURL.to_string());
        }
        if item.setData_forType(&bytes, &own_type) {
            written.push(image.uti.to_string());
        }
        if let Some(rendered) = NSImage::initWithData(NSImage::alloc(), &bytes) {
            if let Some(tiff) = rendered.TIFFRepresentation() {
                if image.uti != "public.png"
                    && let Some(png) = NSBitmapImageRep::imageRepWithData(&tiff).and_then(|rep| {
                        rep.representationUsingType_properties(
                            NSBitmapImageFileType::PNG,
                            &NSDictionary::new(),
                        )
                    })
                    && item.setData_forType(&png, NSPasteboardTypePNG)
                {
                    written.push(NSPasteboardTypePNG.to_string());
                }
                if item.setData_forType(&tiff, NSPasteboardTypeTIFF) {
                    written.push(NSPasteboardTypeTIFF.to_string());
                }
            }
        } else {
            return Err(format!("AppKit can't decode this {} image", image.uti));
        }
    }

    pasteboard.clearContents();
    let objects = NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]);
    if !pasteboard.writeObjects(&objects) {
        return Err("The pasteboard refused the image".into());
    }
    Ok(written)
}

/// Presses ⌘V as if on the keyboard, so the app in front pastes. Needs
/// Accessibility access; without it macOS silently drops the events.
pub fn press_paste_shortcut() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "Could not create a keyboard event source".to_string())?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|_| "Could not create the key down event".to_string())?;
    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|_| "Could not create the key up event".to_string())?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);
    Ok(())
}

#[cfg(test)]
mod tests {
    use objc2::rc::Retained;

    use super::*;

    /// On a private pasteboard, so the user's clipboard is left alone.
    fn write_sample(name: &str, uti: &str, bytes: &[u8]) -> (Retained<NSPasteboard>, Vec<String>) {
        let file = std::env::temp_dir().join(format!("quick-search-test-{name}"));
        std::fs::write(&file, bytes).unwrap();
        let pasteboard = NSPasteboard::pasteboardWithUniqueName();
        let written = write_image(
            ImageForClipboard {
                bytes,
                uti,
                file: &file,
            },
            &pasteboard,
        )
        .unwrap();
        (pasteboard, written)
    }

    #[test]
    fn a_gif_gets_file_url_own_bytes_png_and_tiff() {
        let bytes = include_bytes!("../../src/quick-search/images/sample-animated.gif");
        let (pasteboard, written) = write_sample("a.gif", "com.compuserve.gif", bytes);

        for expected in [
            "public.file-url",
            "com.compuserve.gif",
            "public.png",
            "public.tiff",
        ] {
            assert!(
                written.contains(&expected.to_string()),
                "{written:?} lacks {expected}"
            );
        }
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
        let bytes = include_bytes!("../../src/quick-search/images/sample.webp");
        let (_pasteboard, written) = write_sample("a.webp", "org.webmproject.webp", bytes);
        assert!(written.contains(&"public.png".to_string()), "{written:?}");
        assert!(written.contains(&"public.tiff".to_string()), "{written:?}");
    }

    #[test]
    fn a_png_is_not_rendered_to_png_again() {
        let bytes = include_bytes!("../../src/quick-search/images/sample.png");
        let (_pasteboard, written) = write_sample("a.png", "public.png", bytes);
        assert_eq!(written.iter().filter(|t| *t == "public.png").count(), 1);
        assert!(written.contains(&"public.tiff".to_string()), "{written:?}");
    }
}
