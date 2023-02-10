use super::*;

mod expr;
mod input;
mod md;
mod plot;
mod template;

pub fn build() -> Result<()> {
    // Timing.
    let timer = Instant::now();

    // Find posts.
    let posts = input::find_posts()?;

    // Build posts.
    let templater = template::Templater::new()?;
    let articles = {
        let mut articles = String::new();
        let post_count = posts.len();
        for (index, post) in posts.into_iter().enumerate() {
            let (html, meta) = md::to_html(&post.markdown, &post.dir_name, &templater);
            let html = expr::render_math(&html)?;
            let html = templater.post(
                &meta.title,
                &post.link_name,
                &post.date,
                &html,
                &meta.commit,
            )?;
            articles.push_str(&html);
            if index < post_count - 1 {
                articles.push_str("<hr>");
            }
            info!("Processed {}", post.link_name);
        }
        articles
    };

    // Finish index.html.
    {
        use std::io::Write;
        let index = templater.body(&articles)?;
        File::create(index_html_path())?.write_all(index.as_bytes())?;
    }

    info!(
        "Blog generation took {} seconds",
        timer.elapsed().as_secs_f64()
    );

    Ok(())
}

pub fn plot() -> Result<()> {
    let timer = Instant::now();
    plot::gen()?;
    info!(
        "Blog plot generation took {} seconds",
        timer.elapsed().as_secs_f64()
    );
    Ok(())
}

fn index_html_path() -> PathBuf {
    blog_path().join("index.html")
}

fn posts_dir() -> PathBuf {
    blog_path().join("posts")
}

fn blog_path() -> PathBuf {
    manifest_dir().join("docs/blog")
}
