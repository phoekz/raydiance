use super::*;

fn unescape_expression(expr: &str) -> String {
    // Todo: markdown crate escapes special characters if they are found in a
    // math expression. This function essentially undoes that behavior, but it
    // shouldn't be necessary in the first place.
    expr.replace("&amp;", "&") // Fix ampersands
        .replace("\r\n", "\n") // Canocalize windows newlines
        .replace("\\\n", "\\\\") // Fix backslashes
}

fn render_inner(html: &str, delim: &str, opts: &katex::Opts) -> Result<String> {
    let mut new_html = String::new();
    let matches = html.match_indices(delim).collect::<Vec<_>>();
    ensure!(matches.len() % 2 == 0);
    let mut cursor = 0;
    for chunk in matches.chunks_exact(2) {
        let (start, _) = chunk[0];
        let (end, _) = chunk[1];
        new_html.push_str(&html[cursor..start]);
        let math = &html[(start + delim.len())..end];
        let math = unescape_expression(math);
        let math = katex::render_with_opts(&math, opts)?;
        new_html.push_str(&math);
        cursor = end + delim.len();
    }
    new_html.push_str(&html[cursor..]);
    Ok(new_html)
}

fn render_display_math(html: &str) -> Result<String> {
    let opts = katex::Opts::builder()
        .display_mode(true)
        .output_type(katex::OutputType::Html)
        .trust(true)
        .build()?;
    render_inner(html, "$$", &opts)
}

fn render_inline_math(html: &str) -> Result<String> {
    let opts = katex::Opts::builder()
        .display_mode(false)
        .output_type(katex::OutputType::Html)
        .trust(true)
        .build()?;
    render_inner(html, "$", &opts)
}

pub fn render_math(html: &str) -> Result<String> {
    let html = render_display_math(html)?;
    let html = render_inline_math(&html)?;
    Ok(html)
}
