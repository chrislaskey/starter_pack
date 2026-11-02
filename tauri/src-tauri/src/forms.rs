//! The Phoenix app's form definitions, compiled in by path from the
//! `.dynamic_form.json` files `mix app.dynamic_forms` writes beside the
//! LiveViews that define them, and validated and shown with the
//! `dynamic_form` crate, which follows the `dynamic_form` library the
//! Phoenix app renders them with. See phoenix/docs/design/dynamic-forms.md.
//!
//! The page sends a form's `FormData` entries as they are, with the state
//! the form last showed with; the crate decodes them the way Plug decodes
//! them on the server, so the same form reaches the same rules as the
//! same params, and runs the form as the library's LiveComponent does.

use dynamic_form::{Choice, Definition, DynamicForm, FormState, Shown};
use serde_json::Value;

use crate::sync::store::Community;

/// `App.Web.PostsLive.post_form/0`'s definition.
const POST_FORM_JSON: &str =
    include_str!("../../../phoenix/lib/app/web/live/post_form.dynamic_form.json");

/// The post form's `<form>` id and the name its params are under, as
/// `post[body]`.
pub const POST_FORM_ID: &str = "post-form";
pub const POST_FORM_NAME: &str = "post";

/// What a valid post form says to write.
#[derive(Debug, PartialEq)]
pub struct NewPost {
    pub body: String,
    pub community_ids: Vec<String>,
}

/// The post form, with the communities as the choices of `community_ids`,
/// as `App.Web.PostsLive.post_form/1` makes it.
pub fn post_form(communities: &[Community]) -> Result<Definition, String> {
    let mut definition = Definition::parse(POST_FORM_JSON)?;
    let choices = communities
        .iter()
        .map(|community| Choice::labeled(community.name.clone(), community.id.clone()))
        .collect();
    definition.put_choices("community_ids", choices)?;
    Ok(definition)
}

/// The empty post form, as it first shows.
pub fn show_post_form(communities: &[Community]) -> Result<Shown, String> {
    let definition = post_form(communities)?;
    Ok(DynamicForm::new(&definition, POST_FORM_ID, POST_FORM_NAME).mount())
}

/// The post form after a change, from its `FormData` entries and the
/// state it last showed with.
pub fn change_post_form(
    communities: &[Community],
    state: &FormState,
    entries: &[(String, String)],
) -> Result<Shown, String> {
    let definition = post_form(communities)?;
    DynamicForm::new(&definition, POST_FORM_ID, POST_FORM_NAME).change(state, entries)
}

/// The post to write when the submitted form is valid, or the form with
/// its errors showing when it isn't.
pub fn submit_post_form(
    communities: &[Community],
    state: &FormState,
    entries: &[(String, String)],
) -> Result<Result<NewPost, Shown>, String> {
    let definition = post_form(communities)?;
    let data =
        match DynamicForm::new(&definition, POST_FORM_ID, POST_FORM_NAME).submit(state, entries)? {
            Ok(data) => data,
            Err(shown) => return Ok(Err(shown)),
        };

    let body = data
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let community_ids = match data.get("community_ids") {
        Some(Value::Array(ids)) => ids
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    };
    Ok(Ok(NewPost {
        body,
        community_ids,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn communities() -> Vec<Community> {
        vec![Community {
            id: "0194d3b0-0000-7000-8000-000000000001".into(),
            name: "Gardeners".into(),
        }]
    }

    fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    fn field<'a>(shown: &'a Shown, name: &str) -> &'a serde_json::Value {
        shown
            .view
            .elements
            .iter()
            .find_map(|element| match element {
                dynamic_form::ElementView::Field { name: n, props, .. } if n == name => Some(props),
                _ => None,
            })
            .expect("the field")
    }

    #[test]
    fn the_post_form_offers_the_users_communities() {
        let form = show_post_form(&communities()).unwrap();

        assert_eq!(form.view.elements.len(), 2);
        assert_eq!(field(&form, "body")["id"], "post-form_body");
        let options = &field(&form, "community_ids")["options"];
        assert_eq!(options[0]["label"], "Gardeners");
        assert_eq!(options[0]["checked"], false);
    }

    #[test]
    fn errors_wait_for_a_failed_submit_as_on_the_web() {
        let blank = entries(&[("post[body]", " "), ("post[community_ids][]", "")]);
        let shown = show_post_form(&communities()).unwrap();

        let typing = change_post_form(&communities(), &shown.state, &blank).unwrap();
        assert_eq!(field(&typing, "body")["errors"], serde_json::json!([]));

        let submitted = submit_post_form(&communities(), &typing.state, &blank)
            .unwrap()
            .unwrap_err();
        assert_eq!(
            field(&submitted, "body")["errors"],
            serde_json::json!(["can't be blank"])
        );

        let typing_again = change_post_form(&communities(), &submitted.state, &blank).unwrap();
        assert_eq!(
            field(&typing_again, "body")["errors"],
            serde_json::json!(["can't be blank"])
        );
    }

    #[test]
    fn a_valid_post_form_says_what_to_write() {
        let submitted = entries(&[
            ("post[body]", "Hello"),
            ("post[community_ids][]", ""),
            (
                "post[community_ids][]",
                "0194d3b0-0000-7000-8000-000000000001",
            ),
        ]);
        let shown = show_post_form(&communities()).unwrap();

        assert_eq!(
            submit_post_form(&communities(), &shown.state, &submitted)
                .unwrap()
                .unwrap(),
            NewPost {
                body: "Hello".into(),
                community_ids: vec!["0194d3b0-0000-7000-8000-000000000001".into()],
            }
        );
    }

    #[test]
    fn an_invalid_post_form_comes_back_with_its_errors() {
        let long = "x".repeat(10_001);
        let submitted = entries(&[("post[body]", &long)]);
        let shown = show_post_form(&communities()).unwrap();

        let form = submit_post_form(&communities(), &shown.state, &submitted)
            .unwrap()
            .unwrap_err();
        assert!(!form.view.valid);
        assert_eq!(
            field(&form, "body")["errors"],
            serde_json::json!(["should be at most 10000 character(s)"])
        );
        assert_eq!(field(&form, "body")["value"], long);
    }
}
