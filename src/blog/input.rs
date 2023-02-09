use super::*;

#[derive(Debug)]
pub struct Post {
    pub dir_name: String,
    pub timestamp: String,
    pub date: String,
    pub link_name: String,
    pub markdown: String,
}

pub fn find_posts() -> Result<Vec<Post>> {
    // Find and prepare posts for further processing.
    let mut posts = vec![];
    for dir_entry in std::fs::read_dir(posts_dir())? {
        let dir_entry = dir_entry?;
        let file_type = dir_entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }

        let dir_path = dir_entry.path();
        let markdown = read_markdown_file(&dir_path)?;
        let dir_name = dir_path.file_name().unwrap().to_string_lossy();
        ensure!(
            dir_name.len() > 16,
            "Post directory name must have the following format YYYYMMDD-HHMMSS-[link_name]"
        );
        let timestamp = &dir_name[0..15];
        let date = format!(
            "{}-{}-{}",
            &timestamp[0..4],
            &timestamp[4..6],
            &timestamp[6..8]
        );
        let link_name = dir_name[16..].to_owned();
        posts.push(Post {
            dir_name: dir_name.to_string(),
            timestamp: timestamp.to_string(),
            date,
            link_name,
            markdown,
        });
    }

    // Sort by newest to oldest.
    posts.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(posts)
}

fn read_markdown_file(post_dir: &Path) -> Result<String> {
    for dir_entry in std::fs::read_dir(post_dir)? {
        let dir_entry = dir_entry?;
        let file_type = dir_entry.file_type()?;
        if !file_type.is_file() {
            continue;
        }
        let file_path = dir_entry.path();
        let file_name = file_path.file_name().unwrap().to_string_lossy();
        if file_name == "post.md" {
            let file_data = std::fs::read_to_string(file_path)?;
            return Ok(file_data);
        }
    }

    bail!("Could not find post.md under {}", post_dir.display());
}
