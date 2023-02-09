use super::*;

use tinytemplate::TinyTemplate;

pub struct Templater<'a>(TinyTemplate<'a>);

const BLOG_TITLE: &str = "Raydiance - Blog";
const GITHUB_REPOSITORY: &str = "https://github.com/phoekz/raydiance";
const GITHUB_PAGES: &str = "https://phoekz.github.io/raydiance";

impl Templater<'_> {
    pub fn new() -> Result<Self> {
        let mut tt = TinyTemplate::new();
        tt.set_default_formatter(&tinytemplate::format_unescaped);
        tt.add_template("body", include_str!("templates/body.html"))?;
        tt.add_template("post", include_str!("templates/post.html"))?;
        tt.add_template("image-single", include_str!("templates/image-single.html"))?;
        tt.add_template("image-pair", include_str!("templates/image-pair.html"))?;
        tt.add_template("video", include_str!("templates/video.html"))?;
        Ok(Self(tt))
    }

    pub fn post(
        &self,
        title: &str,
        link: &str,
        date: &str,
        content: &str,
        commit: &str,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Context<'a> {
            github: &'a str,
            title: &'a str,
            link: &'a str,
            date: &'a str,
            content: &'a str,
            commit: &'a str,
            commit_short: &'a str,
        }

        let html = self.0.render(
            "post",
            &Context {
                github: GITHUB_REPOSITORY,
                title,
                link,
                date,
                content,
                commit,
                commit_short: &commit[..8],
            },
        )?;

        Ok(html)
    }

    pub fn body(&self, articles: &str) -> Result<String> {
        #[derive(Serialize)]
        struct Context<'a> {
            blog_title: &'a str,
            style: &'a str,
            articles: &'a str,
            github_pages: &'a str,
            github: &'a str,
            copyright: &'a str,
        }

        let html = self.0.render(
            "body",
            &Context {
                blog_title: BLOG_TITLE,
                style: include_str!("templates/style.css"),
                articles,
                github_pages: GITHUB_PAGES,
                github: GITHUB_REPOSITORY,
                copyright: &copyright(),
            },
        )?;

        Ok(html)
    }

    pub fn inner(&self) -> &TinyTemplate<'_> {
        &self.0
    }
}

fn copyright() -> String {
    let utc_time = time::OffsetDateTime::now_utc();
    format!("© {} Vinh Truong", utc_time.year())
}
