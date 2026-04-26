use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteImage {
    pub alt: String,
    pub href: String,
    pub resolved: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteBlock {
    Markdown(String),
    Image {
        href: String,
        alt: String,
        resolved: Option<PathBuf>,
    },
    Formula(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub path: PathBuf,
    pub title: String,
    pub tags: Vec<String>,
    pub blocks: Vec<NoteBlock>,
    pub images: Vec<NoteImage>,
}
