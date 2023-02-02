use super::*;

use tinytemplate::TinyTemplate;

fn posts_dir() -> PathBuf {
    manifest_dir().join("src/blog/posts")
}

fn index_html_path() -> PathBuf {
    manifest_dir().join("docs/blog/index.html")
}

fn get_posts() -> Result<Vec<PathBuf>> {
    let mut posts = vec![];
    for path in std::fs::read_dir(posts_dir())? {
        let path = path?.path();
        if path.is_file() {
            let ext = path
                .extension()
                .unwrap_or_else(|| panic!("{} must have an extension", path.display()))
                .to_string_lossy();
            assert_eq!(ext, "md", "{} must end with .md", path.display());
            posts.push(path);
        }
    }

    Ok(posts)
}

#[derive(Debug)]
struct PostInfo {
    title: String,
    link: String,
    date: String,
    commit: String,
}

impl PostInfo {
    fn try_parse(markdown: &str) -> Result<Self> {
        use markdown::mdast::AttributeContent;
        use markdown::mdast::AttributeValue;
        use markdown::mdast::MdxJsxAttribute;
        use markdown::mdast::Node;

        fn get_property(attribute: &AttributeContent) -> Result<&MdxJsxAttribute> {
            if let AttributeContent::Property(property) = attribute {
                Ok(property)
            } else {
                bail!("Only properties are supported");
            }
        }

        fn get_literal(value: &Option<AttributeValue>) -> Result<String> {
            let date = value.as_ref().unwrap();
            match date {
                markdown::mdast::AttributeValue::Literal(date) => Ok(date.into()),
                markdown::mdast::AttributeValue::Expression(_) => {
                    bail!("Only literal attribute values are supported");
                }
            }
        }

        let root =
            markdown::to_mdast(markdown, &markdown::ParseOptions::mdx()).map_err(|e| anyhow!(e))?;
        let nodes = root
            .children()
            .context("Root must contain at least one children")?;
        let mut title: Option<String> = None;
        let mut link: Option<String> = None;
        let mut date: Option<String> = None;
        let mut commit: Option<String> = None;
        for node in nodes {
            if let Node::MdxJsxFlowElement(element) = node {
                let name = element
                    .name
                    .as_ref()
                    .ok_or_else(|| anyhow!("Element must have a name"))?;
                if name != "info" {
                    continue;
                }

                for attribute in &element.attributes {
                    let property = get_property(attribute)?;
                    let value = get_literal(&property.value)?;
                    match property.name.as_str() {
                        "title" => {
                            title = Some(value);
                        }
                        "link" => {
                            ensure!(Self::validate_link(&value), "invalid `link`: {}", value);
                            link = Some(value);
                        }
                        "date" => {
                            ensure!(Self::validate_date(&value), "invalid `date`: {}", value);
                            date = Some(value);
                        }
                        "commit" => {
                            ensure!(Self::validate_commit(&value), "invalid `commit`: {}", value);
                            commit = Some(value);
                        }
                        property => {
                            panic!("Unexpected property: {property}");
                        }
                    }
                }
            }
        }

        Ok(PostInfo {
            title: title.ok_or_else(|| anyhow!("Posts must define `title` inside <info>"))?,
            link: link.ok_or_else(|| anyhow!("Posts must define `link` inside <info>"))?,
            date: date.ok_or_else(|| anyhow!("Posts must define `date` inside <info>"))?,
            commit: commit.ok_or_else(|| anyhow!("Posts must define `commit` inside <info>"))?,
        })
    }

    fn validate_link(link: &str) -> bool {
        link.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    fn validate_date(date: &str) -> bool {
        let format = time::format_description::parse("[year]-[month]-[day]")
            .expect("Failed to parse format description");
        time::Date::parse(date, &format).is_ok()
    }

    fn validate_commit(commit: &str) -> bool {
        commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit())
    }
}

fn katex_unescape_expression(expr: &str) -> String {
    // Todo: markdown crate escapes special characters if they are found in a
    // math expression. This function essentially undoes that behavior, but it
    // shouldn't be necessary in the first place.
    expr.replace("&amp;", "&") // Fix ampersands
        .replace("\r\n", "\n") // Canocalize windows newlines
        .replace("\\\n", "\\\\") // Fix backslashes
}

fn katex_render_inner(html: &str, delim: &str, opts: &katex::Opts) -> Result<String> {
    let mut new_html = String::new();
    let matches = html.match_indices(delim).collect::<Vec<_>>();
    ensure!(matches.len() % 2 == 0);
    let mut cursor = 0;
    for chunk in matches.chunks_exact(2) {
        let (start, _) = chunk[0];
        let (end, _) = chunk[1];
        new_html.push_str(&html[cursor..start]);
        let math = &html[(start + delim.len())..end];
        let math = katex_unescape_expression(math);
        let math = katex::render_with_opts(&math, opts)?;
        new_html.push_str(&math);
        cursor = end + delim.len();
    }
    new_html.push_str(&html[cursor..]);
    Ok(new_html)
}

fn katex_render_display_math(html: &str) -> Result<String> {
    let opts = katex::Opts::builder()
        .display_mode(true)
        .output_type(katex::OutputType::Html)
        .trust(true)
        .build()?;
    katex_render_inner(html, "$$", &opts)
}

fn katex_render_inline_math(html: &str) -> Result<String> {
    let opts = katex::Opts::builder()
        .display_mode(false)
        .output_type(katex::OutputType::Html)
        .trust(true)
        .build()?;
    katex_render_inner(html, "$", &opts)
}

fn katex_render_math(html: &str) -> Result<String> {
    let html = katex_render_display_math(html)?;
    let html = katex_render_inline_math(&html)?;
    Ok(html)
}

pub fn build() -> Result<()> {
    use std::io::Write;

    #[derive(serde::Serialize)]
    struct BodyContext<'a> {
        blog_title: &'a str,
        style: &'a str,
        articles: &'a str,
        github_pages: &'a str,
        github: &'a str,
        copyright: &'a str,
    }

    #[derive(serde::Serialize)]
    struct PostContext<'a> {
        github: &'a str,
        title: &'a str,
        link: &'a str,
        date: &'a str,
        content: &'a str,
        commit: &'a str,
        commit_short: &'a str,
    }

    let blog_title = "Raydiance - Blog";
    let github = "https://github.com/phoekz/raydiance";
    let github_pages = "https://phoekz.github.io/raydiance";
    let copyright = {
        let local_time = time::OffsetDateTime::now_local().expect("Failed to get local time");
        format!("© {} Vinh Truong", local_time.year())
    };

    // Prepare templater.
    let tt = {
        let mut tt = TinyTemplate::new();
        tt.set_default_formatter(&tinytemplate::format_unescaped);
        tt.add_template("body", include_str!("templates/body.html"))?;
        tt.add_template("post", include_str!("templates/post.html"))?;
        tt
    };

    // Get sorted posts.
    let posts = {
        // Find posts.
        let mut posts = get_posts()?;
        ensure!(!posts.is_empty(), "There must be at least one post");

        // Sort posts in reverse date order (latest post is the first in the blog).
        posts.sort_by(|a, b| {
            let a = a.file_name().unwrap().to_string_lossy();
            let b = b.file_name().unwrap().to_string_lossy();
            b.cmp(&a)
        });

        posts
    };

    // Render posts.
    let posts = posts
        .into_iter()
        .map(|path| {
            let md = std::fs::read_to_string(path)?;
            let md_options = markdown::Options {
                parse: markdown::ParseOptions {
                    constructs: markdown::Constructs {
                        math_flow: false,
                        math_text: false,
                        ..markdown::Constructs::default()
                    },
                    ..markdown::ParseOptions::mdx()
                },
                compile: markdown::CompileOptions {
                    allow_dangerous_html: true,
                    ..markdown::CompileOptions::default()
                },
            };
            let html = markdown::to_html_with_options(&md, &md_options).map_err(|e| anyhow!(e))?;
            let html = katex_render_math(&html)?;
            let info = PostInfo::try_parse(&md)?;
            let html = tt.render(
                "post",
                &PostContext {
                    github,
                    title: &info.title,
                    link: &info.link,
                    date: &info.date,
                    content: &html,
                    commit: &info.commit,
                    commit_short: &info.commit[..8],
                },
            )?;
            Ok(html)
        })
        .collect::<Result<Vec<_>>>()?;
    info!("Rendered {} posts", posts.len());

    // Combine posts into <article>s.
    let articles = {
        let mut articles = String::new();
        let post_count = posts.len();
        for (index, post) in posts.into_iter().enumerate() {
            articles.push_str(&post);
            if index < post_count - 1 {
                articles.push_str("<hr>");
            }
        }
        articles
    };

    // Build final body.
    let body = tt.render(
        "body",
        &BodyContext {
            blog_title,
            style: include_str!("templates/style.css"),
            articles: &articles,
            github_pages,
            github,
            copyright: &copyright,
        },
    )?;

    // Write to index.html.
    {
        let mut file = File::create(index_html_path())?;
        file.write_all(body.as_bytes())?;
    }

    Ok(())
}
