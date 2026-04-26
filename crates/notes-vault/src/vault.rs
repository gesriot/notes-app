use crate::{
    Note,
    frontmatter::{FrontMatter, normalize_tags},
    parser::{extract_images, parse_blocks},
};
use gray_matter::{Matter, engine::YAML};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Vault {
    pub root: PathBuf,
    pub notes_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse frontmatter in {path}: {message}")]
    Frontmatter { path: PathBuf, message: String },
    #[error("failed to walk notes directory: {0}")]
    WalkDir(String),
}

impl Vault {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let root = root.canonicalize().unwrap_or(root);
        let notes_dir = root.join("notes");

        Self { root, notes_dir }
    }

    pub fn load_notes(&self) -> Result<Vec<Note>, VaultError> {
        if !self.notes_dir.exists() {
            return Ok(Vec::new());
        }

        let mut paths = Vec::new();
        for entry in WalkDir::new(&self.notes_dir) {
            let entry = entry.map_err(|error| VaultError::WalkDir(error.to_string()))?;
            if entry.file_type().is_file()
                && entry.path().extension().is_some_and(|ext| ext == "md")
            {
                paths.push(entry.path().to_path_buf());
            }
        }

        let mut notes = paths
            .iter()
            .map(|path| self.load_note(path))
            .collect::<Result<Vec<_>, _>>()?;

        notes.sort_by_key(|note| {
            note.path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_lowercase()
        });

        Ok(notes)
    }

    fn load_note(&self, path: &Path) -> Result<Note, VaultError> {
        let raw_text = fs::read_to_string(path).map_err(|source| VaultError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        let matter = Matter::<YAML>::new();
        let parsed =
            matter
                .parse::<FrontMatter>(&raw_text)
                .map_err(|error| VaultError::Frontmatter {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;

        let frontmatter = parsed.data.unwrap_or_default();
        let body = parsed.content.trim();
        let title = frontmatter.title.unwrap_or_else(|| fallback_title(path));
        let tags = normalize_tags(frontmatter.tags);
        let images = extract_images(path, body);
        let blocks = parse_blocks(path, body);
        let relative_path = path.strip_prefix(&self.root).unwrap_or(path).to_path_buf();

        Ok(Note {
            path: relative_path,
            title,
            tags,
            blocks,
            images,
        })
    }
}

fn fallback_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled")
        .replace('-', " ")
        .replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::fallback_title;
    use std::path::Path;

    #[test]
    fn fallback_title_uses_file_stem() {
        assert_eq!(
            fallback_title(Path::new("01-example_note.md")),
            "01 example note"
        );
    }
}
