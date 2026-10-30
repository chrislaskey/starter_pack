//! The Phoenix app's universal changesets, compiled in by path from the
//! Rust `mix app.rust` writes next to each schema, so a record is checked
//! here with the rules the server runs and a write that would be refused
//! on push is refused before it is saved. See phoenix/priv/rust/README.md.
//!
//! A changeset takes `data`, the row as it is plus what the app sets, and
//! `params`, what the user typed. A form's entries go through
//! `ecto_changeset::form::decode` first, so the same form reaches the same
//! rules as the same params as on the server.

use ecto_changeset::Changeset;

/// `App.Data.Post.changeset/3`. The app sets `owner_id`, so it goes in
/// `data`, as the posting flow sets it on the server.
#[allow(dead_code)]
#[path = "../../../phoenix/lib/app/data/schemas/posts/post.rs"]
pub mod post;

/// `App.Data.UserProfile.changeset/3`. The app sets `user_id`.
#[allow(dead_code)]
#[path = "../../../phoenix/lib/app/data/schemas/user_profiles/user_profile.rs"]
pub mod user_profile;

/// The changeset when it is valid, or its errors as one line for the
/// page, like `body can't be blank`.
pub fn checked(changeset: Changeset) -> Result<Changeset, String> {
    if changeset.valid() {
        return Ok(changeset);
    }
    Err(changeset
        .errors()
        .iter()
        .map(|error| format!("{} {}", error.field, error.text()))
        .collect::<Vec<_>>()
        .join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecto_changeset::Params;
    use serde_json::json;

    fn object(value: serde_json::Value) -> Params {
        value.as_object().cloned().expect("an object")
    }

    #[test]
    fn a_post_is_checked_with_the_servers_rules() {
        let owner = json!({"owner_id": "01a17558-0000-7000-8000-000000000001"});

        let valid = post::changeset(&object(owner.clone()), &object(json!({"body": "Hello"})));
        assert!(checked(valid).is_ok());

        let errors = checked(post::changeset(
            &object(owner),
            &object(json!({"body": " "})),
        ));
        assert_eq!(errors.unwrap_err(), "body can't be blank");

        let no_owner = post::changeset(&Params::new(), &object(json!({"body": "Hello"})));
        assert_eq!(checked(no_owner).unwrap_err(), "owner_id can't be blank");
    }
}
