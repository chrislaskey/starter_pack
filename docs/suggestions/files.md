# Files: what was left out

Built from `docs/design/offline-files-decisions.md`: a `files` resource
whose rows sync like posts, whose bytes are kept on the device that
added them and sent to the bucket by the sync, fetched by another
device the first time it shows the file, with a limit of tries, refused
files kept for the person, and the cleanups on both sides. Left for
later, on purpose:

- **Scheduling the sweep.** `files::delete_unreferenced` waits for a
  scheduler beside `avatars::delete_unreferenced`, to run daily after
  `app_web purge`.
- **Communities for files**, by the join table posts use, with the
  backfill and the prune that come with visibility.
- **Replacing a file's content.** The bytes are fixed today; replacing
  them needs a key per content and a stored key that merges like a
  field.
- **Uploads in a task of their own.** `files::upload_waiting` runs
  inside the sync and holds its lock while bytes go. An app with large
  files calls it from its own task, then starts a sync; no data changes.
- **Multipart and resumable uploads.** One PUT per file, started over
  when cut off.
- **Uploads while the app is in the background.** A phone suspends the
  app; the upload starts over when it comes back.
- **Audio and video.** WebKit asks for a media file in ranges, which
  the `files` protocol would have to answer.
- **More types shown in place.** `files::ShownAs` knows images, plain
  text and PDF.
- **Download on phones**, through the system's share sheet, which
  needs a plugin.
- **Copying an image on Windows, Linux and Android.** macOS, iOS and
  the web are built. On Windows and Linux,
  `tauri-plugin-clipboard-manager`'s `write_image`, a still, or the
  platform's own formats for an animation. On Android, a `content://`
  URI from a FileProvider, in a small Kotlin plugin.
- **Thumbnails.** The index shows each image whole, scaled down by the
  page, so it fetches every image's full bytes.
- **A checksum.** The server checks a file's size only.
- **A file attached to a post.**
- **Deleting an account's files at once.** There is no deleting an
  account yet; when there is, it deletes the prefix `files/<owner id>/`
  without waiting for the sweep.
