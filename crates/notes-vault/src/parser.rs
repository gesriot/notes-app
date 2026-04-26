use crate::{NoteBlock, NoteImage};
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

static IMAGE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!\[(?P<alt>[^\]]*)\]\((?P<href>[^)]+)\)").unwrap());

static BLOCK_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ms)(?P<fence>^```(?:latex|math)\n(?P<fence_body>.*?)\n```[ \t]*\n?)|(?P<display>^\$\$\n?(?P<display_body>.*?)\n?\$\$[ \t]*\n?)|(?P<image>^(?P<image_line>!\[(?P<image_alt>[^\]]*)\]\((?P<image_href>[^)]+)\))[ \t]*\n?)",
    )
    .unwrap()
});

pub fn extract_images(note_path: &Path, body: &str) -> Vec<NoteImage> {
    IMAGE_PATTERN
        .captures_iter(body)
        .map(|capture| {
            let href = capture
                .name("href")
                .map(|value| value.as_str().trim().to_owned())
                .unwrap_or_default();
            let alt = capture
                .name("alt")
                .map(|value| value.as_str().trim().to_owned())
                .unwrap_or_default();

            NoteImage {
                resolved: resolve_link(note_path, &href),
                href,
                alt,
            }
        })
        .collect()
}

pub fn parse_blocks(note_path: &Path, body: &str) -> Vec<NoteBlock> {
    let mut blocks = Vec::new();
    let mut position = 0;

    for capture in BLOCK_PATTERN.captures_iter(body) {
        let Some(matched) = capture.get(0) else {
            continue;
        };

        if matched.start() > position {
            push_markdown(&mut blocks, &body[position..matched.start()]);
        }

        if let Some(href) = capture.name("image_href") {
            let href = href.as_str().trim().to_owned();
            let alt = capture
                .name("image_alt")
                .map(|value| value.as_str().trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| {
                    Path::new(&href)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(&href)
                        .to_owned()
                });

            blocks.push(NoteBlock::Image {
                resolved: resolve_link(note_path, &href),
                href,
                alt,
            });
        } else {
            let formula = capture
                .name("fence_body")
                .or_else(|| capture.name("display_body"))
                .map(|value| value.as_str().trim().to_owned())
                .unwrap_or_default();

            if !formula.is_empty() {
                blocks.push(NoteBlock::Formula(formula));
            }
        }

        position = matched.end();
    }

    if position < body.len() {
        push_markdown(&mut blocks, &body[position..]);
    }

    blocks
}

pub fn resolve_link(note_path: &Path, href: &str) -> Option<PathBuf> {
    if href.contains("://") {
        return None;
    }

    let parent = note_path.parent().unwrap_or_else(|| Path::new(""));
    let candidate = parent.join(href.trim());
    if !candidate.exists() {
        return None;
    }

    Some(candidate.canonicalize().unwrap_or(candidate))
}

fn push_markdown(blocks: &mut Vec<NoteBlock>, markdown: &str) {
    let markdown = markdown.trim();
    if !markdown.is_empty() {
        blocks.push(NoteBlock::Markdown(markdown.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_images, parse_blocks};
    use crate::NoteBlock;
    use std::path::Path;

    #[test]
    fn extracts_all_images_from_body() {
        let images = extract_images(
            Path::new("/tmp/vault/notes/example.md"),
            "Text ![inline](../images/a.png)\n\n![block](../images/b.jpg)",
        );

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].alt, "inline");
        assert_eq!(images[1].href, "../images/b.jpg");
    }

    #[test]
    fn splits_markdown_images_and_formulas() {
        let blocks = parse_blocks(
            Path::new("/tmp/vault/notes/example.md"),
            "# Title\n\n![Graph](../images/graph.jpg)\n\n$$\na + b\n$$\n\nDone",
        );

        assert_eq!(blocks.len(), 4);
        assert!(matches!(blocks[0], NoteBlock::Markdown(_)));
        assert!(matches!(blocks[1], NoteBlock::Image { .. }));
        assert_eq!(blocks[2], NoteBlock::Formula("a + b".to_owned()));
        assert_eq!(blocks[3], NoteBlock::Markdown("Done".to_owned()));
    }

    #[test]
    fn supports_latex_fences() {
        let blocks = parse_blocks(
            Path::new("/tmp/vault/notes/example.md"),
            "```latex\n\\frac{a}{b}\n```\n",
        );

        assert_eq!(blocks, vec![NoteBlock::Formula("\\frac{a}{b}".to_owned())]);
    }
}
