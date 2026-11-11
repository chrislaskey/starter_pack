# Multi-app: suggestions

Not built, to discuss first:

- Changing a name on the profile page. A name is set at sign-up only;
  the change is a write to `app_users`, so it is an event, online from
  a device as the picture is, or queued offline as a post is.
- Deleting an account: `accounts_users.deleted_at` is there for it, and
  the partial index frees the email, but nothing sets it yet.
- Serving `ops.html` only to staff, so even the page is not public, and
  leaving the ops entry out of the device bundles.
- A second staff role, or permissions per tool, once ops has tools.
- `app_web ops staff list`.
