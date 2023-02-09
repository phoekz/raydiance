use super::*;

use pulldown_cmark::{html, Event, LinkType, Options, Parser, Tag};

pub fn to_html(markdown: &str, dir_name: &str, tt: &template::Templater<'_>) -> (String, Metadata) {
    let options = Options::empty();
    let ctx = EventContext { dir: dir_name, tt };
    let mut events = vec![];
    let mut meta: Option<Metadata> = None;
    for event in Parser::new_ext(markdown, options) {
        let (event, event_meta) = ctx.process_event(event);
        if meta.is_none() && event_meta.is_some() {
            meta = event_meta;
        }
        events.push(event);
    }
    let meta = meta.unwrap_or_else(|| panic!("Could not parse Meta element from post {dir_name}"));
    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    (html_output, meta)
}

struct EventContext<'a> {
    dir: &'a str,
    tt: &'a template::Templater<'a>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub title: String,
    pub commit: String,
}

impl EventContext<'_> {
    #[allow(clippy::match_same_arms)]
    fn process_event<'a>(&'a self, event: Event<'a>) -> (Event<'a>, Option<Metadata>) {
        match event.clone() {
            Event::Start(tag) => {
                if let Tag::Image(link_type, url, _) = tag {
                    (self.process_image(link_type, &url), None)
                } else {
                    (event, None)
                }
            }
            Event::Text(text) => self.process_text(&text),
            _ => (event, None),
        }
    }

    fn process_image(&self, link_type: LinkType, url: &str) -> Event {
        let rebased_url = self.rebase_url(url);
        if url.starts_with("media/") {
            info!("{link_type:?} {rebased_url:?}");
        }
        Event::Start(Tag::Image(link_type, rebased_url.into(), "".into()))
    }

    fn process_text(&self, text: &str) -> (Event, Option<Metadata>) {
        #[derive(Serialize, Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        enum Element {
            Meta(Metadata),
            ImageSingle(ImageSingle),
            ImagePair(ImagePair),
            Video(Video),
            InlineSvg(InlineSvg),
        }

        #[derive(Serialize, Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        struct ImageSingle {
            image: String,
            text: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        struct ImagePair {
            left: String,
            left_text: String,
            right: String,
            right_text: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        struct Video {
            h265: String,
            vp9: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        struct InlineSvg {
            svg: String,
        }

        let is_custom_element = text.starts_with("{{") && text.ends_with("}}");
        if !is_custom_element {
            return (Event::Text(text.to_owned().into()), None);
        }

        let element: Element = {
            // Hack: absolutely make sure the string is escaped for Ron.
            let text = text.replace('\\', "\\\\");
            ron::from_str(&text[2..text.len() - 2]).expect("Failed to parse custom element")
        };
        match element {
            Element::ImageSingle(mut payload) => {
                payload.image = self.rebase_url(&payload.image);
                payload.text = unescape_text(&payload.text);
                let html = self.tt.inner().render("image-single", &payload).unwrap();
                return (Event::Html(html.into()), None);
            }
            Element::ImagePair(mut payload) => {
                payload.left = self.rebase_url(&payload.left);
                payload.right = self.rebase_url(&payload.right);
                payload.left_text = unescape_text(&payload.left_text);
                payload.right_text = unescape_text(&payload.right_text);
                let html = self.tt.inner().render("image-pair", &payload).unwrap();
                return (Event::Html(html.into()), None);
            }
            Element::Video(mut payload) => {
                payload.h265 = self.rebase_url(&payload.h265);
                payload.vp9 = self.rebase_url(&payload.vp9);
                let html = self.tt.inner().render("video", &payload).unwrap();
                return (Event::Html(html.into()), None);
            }
            Element::InlineSvg(mut payload) => {
                payload.svg = self.rebase_url(&payload.svg);
                let svg = std::fs::read_to_string(blog_path().join(payload.svg))
                    .expect("Failed to load inline svg");
                return (Event::Html(svg.into()), None);
            }
            Element::Meta(payload) => {
                return (Event::Html("".into()), Some(payload));
            }
        }
    }

    fn rebase_url(&self, url: &str) -> String {
        format!("posts/{}/{url}", self.dir)
    }
}

fn unescape_text(text: &str) -> String {
    // Hack: If markdown parser encounters anything that looks like HTML tags,
    // it will automatically create a new "event". This means if we need a <br>
    // inside one of our custom elements, we have to unescape back to
    // angle-brackets. Find out if there are better mechanisms for injecting
    // custom elements.
    text.replace("{{br}}", "<br/>")
}
