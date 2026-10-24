//! The items the popup lists: text snippets and sample images. The images
//! are the files under `src/public/quick-search/images/`, which the app's
//! pages serve for the thumbnails.

use serde::Serialize;

const IMAGES_URL: &str = "/quick-search/images";

#[derive(Clone, Copy, Debug)]
pub enum Content {
    Text(&'static str),
    Image {
        file_name: &'static str,
        /// The uniform type identifier of the encoding, like `public.png`.
        uti: &'static str,
        bytes: &'static [u8],
    },
}

pub struct SampleItem {
    pub id: &'static str,
    pub label: &'static str,
    pub content: Content,
}

pub const SAMPLE_ITEMS: [SampleItem; 8] = [
    SampleItem {
        id: "shrug",
        label: "Shrug",
        content: Content::Text(r"¯\_(ツ)_/¯"),
    },
    SampleItem {
        id: "done",
        label: "Done checkmark",
        content: Content::Text("✅ Done"),
    },
    SampleItem {
        id: "lorem",
        label: "Lorem ipsum",
        content: Content::Text("Lorem ipsum dolor sit amet, consectetur adipiscing elit."),
    },
    SampleItem {
        id: "png",
        label: "PNG image",
        content: Content::Image {
            file_name: "sample.png",
            uti: "public.png",
            bytes: include_bytes!("../../../src/public/quick-search/images/sample.png"),
        },
    },
    SampleItem {
        id: "jpeg",
        label: "JPEG image",
        content: Content::Image {
            file_name: "sample.jpg",
            uti: "public.jpeg",
            bytes: include_bytes!("../../../src/public/quick-search/images/sample.jpg"),
        },
    },
    SampleItem {
        id: "gif",
        label: "GIF image",
        content: Content::Image {
            file_name: "sample-static.gif",
            uti: "com.compuserve.gif",
            bytes: include_bytes!("../../../src/public/quick-search/images/sample-static.gif"),
        },
    },
    SampleItem {
        id: "animated-gif",
        label: "Animated GIF",
        content: Content::Image {
            file_name: "sample-animated.gif",
            uti: "com.compuserve.gif",
            bytes: include_bytes!("../../../src/public/quick-search/images/sample-animated.gif"),
        },
    },
    SampleItem {
        id: "webp",
        label: "WebP image",
        content: Content::Image {
            file_name: "sample.webp",
            uti: "org.webmproject.webp",
            bytes: include_bytes!("../../../src/public/quick-search/images/sample.webp"),
        },
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Text,
    Image,
}

/// An item as the popup lists it.
#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ItemKind,
    /// The text itself, or the image's file name and size.
    pub value: String,
    /// The image's URL on the app's own pages, for a thumbnail.
    pub preview_url: Option<String>,
}

impl SampleItem {
    fn item(&self) -> Item {
        match self.content {
            Content::Text(text) => Item {
                id: self.id,
                label: self.label,
                kind: ItemKind::Text,
                value: text.to_string(),
                preview_url: None,
            },
            Content::Image {
                file_name, bytes, ..
            } => Item {
                id: self.id,
                label: self.label,
                kind: ItemKind::Image,
                value: format!("{file_name}, {} KB", bytes.len().div_ceil(1024)),
                preview_url: Some(format!("{IMAGES_URL}/{file_name}")),
            },
        }
    }
}

/// The items whose label or value contains `query`, ignoring case; all of
/// them for an empty query.
pub fn list_items(query: &str) -> Vec<Item> {
    let query = query.trim().to_lowercase();
    SAMPLE_ITEMS
        .iter()
        .map(SampleItem::item)
        .filter(|item| {
            item.label.to_lowercase().contains(&query) || item.value.to_lowercase().contains(&query)
        })
        .collect()
}

pub fn get_sample_item(id: &str) -> Option<&'static SampleItem> {
    SAMPLE_ITEMS.iter().find(|item| item.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_item_for_an_empty_query() {
        let ids: Vec<_> = list_items("").iter().map(|item| item.id).collect();
        assert_eq!(
            ids,
            [
                "shrug",
                "done",
                "lorem",
                "png",
                "jpeg",
                "gif",
                "animated-gif",
                "webp"
            ]
        );
        assert_eq!(
            ids,
            list_items("   ")
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn filters_by_label_or_value_ignoring_case() {
        let by_label: Vec<_> = list_items("GIF").iter().map(|item| item.id).collect();
        assert_eq!(by_label, ["gif", "animated-gif"]);

        let by_value: Vec<_> = list_items("dolor").iter().map(|item| item.id).collect();
        assert_eq!(by_value, ["lorem"]);

        assert!(list_items("nothing like this").is_empty());
    }

    #[test]
    fn text_items_carry_the_text_and_no_preview() {
        let item = &list_items("shrug")[0];
        assert_eq!(item.kind, ItemKind::Text);
        assert_eq!(item.value, r"¯\_(ツ)_/¯");
        assert_eq!(item.preview_url, None);
    }

    #[test]
    fn image_items_carry_the_file_name_size_and_preview_url() {
        let item = &list_items("png")[0];
        assert_eq!(item.kind, ItemKind::Image);
        assert_eq!(item.value, "sample.png, 70 KB");
        assert_eq!(
            item.preview_url.as_deref(),
            Some("/quick-search/images/sample.png")
        );
    }

    #[test]
    fn gets_a_sample_item_by_id() {
        assert_eq!(
            get_sample_item("webp").map(|item| item.label),
            Some("WebP image")
        );
        assert!(get_sample_item("missing").is_none());
    }

    #[test]
    fn image_file_names_exist_on_the_pages() {
        for item in &SAMPLE_ITEMS {
            if let Content::Image { file_name, .. } = item.content {
                let path = concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../src/public/quick-search/images"
                );
                assert!(
                    std::path::Path::new(path).join(file_name).exists(),
                    "{file_name}"
                );
            }
        }
    }
}
