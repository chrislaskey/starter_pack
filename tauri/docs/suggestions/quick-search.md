# Quick search: suggestions

Ideas noted while building the popup (`docs/quick-search.md`), not built.

- **Real items.** The list is eight fixed samples in `items.rs`. Real data
  would come from a command or an API request, with the filter kept in
  Rust.
- **Rebindable shortcut.** Needs `unregister`/`register` on the global
  shortcut plugin and a settings form.
- **Run as an agent.** A Spotlight-like tool usually has no Dock icon:
  `app.set_activation_policy(ActivationPolicy::Accessory)` and a tray icon.
- **Translucent background** like Spotlight: the `macos-private-api`
  feature for a transparent window plus `NSVisualEffectView` through
  `objc2-app-kit` or the `window-vibrancy` crate. Apple may reject the
  private API in App Store review.
- **Pasteboard markers.** Add `org.nspasteboard.source` with the bundle id
  to every copy, and `org.nspasteboard.ConcealedType` if the popup ever
  pastes secrets, so clipboard managers treat them right.
- **Pointer on mixed-DPI displays.** Tauri reports the pointer in the main
  display's physical pixels. Reading `NSEvent.mouseLocation` through
  `objc2-app-kit` would give points directly and remove the conversion.
- **Other clipboard content.** Styled text (`public.rtf`, `public.html`),
  URLs, file lists and PDF are reachable through the clipboard plugin or
  `NSPasteboard`; file promises need a provider object kept alive until
  the paste happens.
- **First responder.** If WebKit stops taking first responder in the
  non-activating panel on some macOS version, call
  `Panel::make_first_responder` on the webview's `NSView` when opening.
