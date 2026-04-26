use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use image::{ImageReader, image_dimensions};
use notes_render::markdown::markdown_to_text;
use notes_vault::{Note, NoteBlock, Vault};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Parser)]
#[command(name = "notes-tui", about = "Terminal vault notes viewer")]
struct Args {
    #[arg(
        long,
        default_value = "vault",
        help = "Path to a vault containing notes/"
    )]
    vault: PathBuf,
}

struct App {
    notes: Vec<Note>,
    list_state: ListState,
    preview_scroll: u16,
    list_items_area: Rect,
    image_hit_areas: Vec<ImageHitArea>,
    fullscreen_image: Option<PathBuf>,
    images: ImageStore,
    preview_cache: HashMap<PathBuf, Vec<PreviewBlock>>,
    should_quit: bool,
}

struct ImageHitArea {
    area: Rect,
    path: PathBuf,
}

struct ImageStore {
    picker: Picker,
    cache: HashMap<PathBuf, CachedImage>,
}

enum CachedImage {
    Ready(StatefulProtocol),
    Failed(String),
}

impl App {
    fn new(vault_root: PathBuf) -> Result<Self> {
        let vault = Vault::new(vault_root);
        let notes = vault
            .load_notes()
            .with_context(|| format!("failed to load vault at {}", vault.root.display()))?;

        let mut list_state = ListState::default();
        if !notes.is_empty() {
            list_state.select(Some(0));
        }
        let image_picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        Ok(Self {
            notes,
            list_state,
            preview_scroll: 0,
            list_items_area: Rect::default(),
            image_hit_areas: Vec::new(),
            fullscreen_image: None,
            images: ImageStore {
                picker: image_picker,
                cache: HashMap::new(),
            },
            preview_cache: HashMap::new(),
            should_quit: false,
        })
    }

    fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    fn next_note(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        let current = self.list_state.selected();
        let next = current
            .map(|i| (i + 1).min(self.notes.len() - 1))
            .unwrap_or(0);
        self.list_state.select(Some(next));
        if current != Some(next) {
            self.preview_scroll = 0;
        }
    }

    fn previous_note(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        let current = self.list_state.selected();
        let prev = current.map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(prev));
        if current != Some(prev) {
            self.preview_scroll = 0;
        }
    }

    fn scroll_preview_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(5);
    }

    fn scroll_preview_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(5);
    }

    fn scroll_preview_by(&mut self, lines: i16) {
        if lines >= 0 {
            self.preview_scroll = self.preview_scroll.saturating_add(lines as u16);
        } else {
            self.preview_scroll = self.preview_scroll.saturating_sub(lines.unsigned_abs());
        }
    }

    fn select_note(&mut self, index: usize) {
        if index >= self.notes.len() {
            return;
        }

        let current = self.list_state.selected();
        self.list_state.select(Some(index));
        if current != Some(index) {
            self.preview_scroll = 0;
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.fullscreen_image.is_some() {
                    return;
                }

                if let Some(path) = self.image_path_at(mouse.column, mouse.row) {
                    self.fullscreen_image = Some(path);
                    return;
                }

                if let Some(index) = self.note_index_at(mouse.column, mouse.row) {
                    self.select_note(index);
                }
            }
            MouseEventKind::ScrollDown if self.fullscreen_image.is_none() => {
                self.scroll_preview_by(3)
            }
            MouseEventKind::ScrollUp if self.fullscreen_image.is_none() => {
                self.scroll_preview_by(-3)
            }
            _ => {}
        }
    }

    fn close_fullscreen_image(&mut self) -> bool {
        self.fullscreen_image.take().is_some()
    }

    fn image_path_at(&self, column: u16, row: u16) -> Option<PathBuf> {
        self.image_hit_areas
            .iter()
            .rev()
            .find(|hit| rect_contains(hit.area, column, row))
            .map(|hit| hit.path.clone())
    }

    fn note_index_at(&self, column: u16, row: u16) -> Option<usize> {
        if !rect_contains(self.list_items_area, column, row) {
            return None;
        }

        let visible_row = row.saturating_sub(self.list_items_area.y) as usize;
        let index = self.list_state.offset().saturating_add(visible_row);
        (index < self.notes.len()).then_some(index)
    }
}

impl ImageStore {
    fn cached_image(&mut self, path: &Path) -> &mut CachedImage {
        if !self.cache.contains_key(path) {
            let image = ImageReader::open(path)
                .map_err(|error| error.to_string())
                .and_then(|reader| reader.decode().map_err(|error| error.to_string()));

            let cached = match image {
                Ok(image) => CachedImage::Ready(self.picker.new_resize_protocol(image)),
                Err(error) => CachedImage::Failed(error),
            };
            self.cache.insert(path.to_path_buf(), cached);
        }

        self.cache
            .get_mut(path)
            .expect("image cache entry inserted before lookup")
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut terminal = ratatui::init();
    crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, App::new(args.vault)?);
    crossterm::execute!(std::io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| draw(frame, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), modifiers)
                            if modifiers.is_empty()
                                || modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.should_quit = true;
                        }
                        (KeyCode::Esc, _) if app.close_fullscreen_image() => {}
                        (KeyCode::Esc, _) => app.should_quit = true,
                        _ if app.fullscreen_image.is_some() => {}
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.next_note(),
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.previous_note(),
                        (KeyCode::PageDown, _) | (KeyCode::Char(' '), _) => {
                            app.scroll_preview_down()
                        }
                        (KeyCode::PageUp, _) | (KeyCode::Char('b'), _) => app.scroll_preview_up(),
                        (KeyCode::Char('d'), modifiers)
                            if modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.scroll_preview_down()
                        }
                        (KeyCode::Char('u'), modifiers)
                            if modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.scroll_preview_up()
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    Ok(())
}

fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(header(), vertical[0]);
    frame.render_widget(search_placeholder(), vertical[1]);
    frame.render_widget(footer(), vertical[3]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(20)])
        .split(vertical[2]);

    render_notes(frame, app, main[0]);
    render_preview(frame, app, main[1]);

    if let Some(path) = app.fullscreen_image.clone() {
        render_fullscreen_image(frame, &mut app.images, &path);
    }
}

fn header() -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled("Vault Notes", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" - поиск и просмотр заметок"),
    ]))
}

fn search_placeholder() -> Paragraph<'static> {
    Paragraph::new("/ Поиск по тегам будет подключен на этапе 5")
        .block(Block::default().borders(Borders::ALL).title("Поиск"))
}

fn footer() -> Paragraph<'static> {
    Paragraph::new(
        "click/↑/↓: заметки  trackpad/Space/b: preview  click image: fullscreen  Esc: закрыть/выход  q: выход",
    )
}

fn render_notes(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let items = app
        .notes
        .iter()
        .map(|note| ListItem::new(note.title.clone()))
        .collect::<Vec<_>>();

    let block = Block::default().borders(Borders::ALL).title(format!(
        "Заметки {}/{}",
        app.notes.len(),
        app.notes.len()
    ));
    app.list_items_area = block.inner(area);

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    // list_state хранится в App, чтобы ratatui мог управлять scroll offset
    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_preview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Preview");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(index) = app.selected_index() else {
        frame.render_widget(
            Paragraph::new("Vault пуст или папка notes/ не найдена"),
            inner,
        );
        return;
    };
    let note_key = app.notes[index].path.clone();
    if !app.preview_cache.contains_key(&note_key) {
        let blocks = preview_blocks(&app.notes[index]);
        app.preview_cache.insert(note_key.clone(), blocks);
    }
    let preview_blocks = app
        .preview_cache
        .get(&note_key)
        .expect("preview cache entry inserted before lookup");

    let visible_height = area.height.saturating_sub(2) as usize;
    let font_size = app.images.picker.font_size();
    let total_height = preview_blocks
        .iter()
        .map(|block| block.height(inner.width, font_size))
        .sum::<u16>() as usize;
    let max_scroll = total_height.saturating_sub(visible_height) as u16;
    app.preview_scroll = app.preview_scroll.min(max_scroll);
    let preview_scroll = app.preview_scroll;
    let images = &mut app.images;
    let image_hit_areas = &mut app.image_hit_areas;
    image_hit_areas.clear();

    render_preview_blocks(
        frame,
        images,
        image_hit_areas,
        preview_scroll,
        inner,
        preview_blocks,
    );
}

enum PreviewBlock {
    Text(Text<'static>),
    Image {
        path: PathBuf,
        href: String,
        alt: String,
        pixel_size: Option<(u32, u32)>,
    },
}

impl PreviewBlock {
    fn height(&self, width: u16, font_size: (u16, u16)) -> u16 {
        match self {
            Self::Text(text) => text.lines.len().max(1) as u16,
            Self::Image { pixel_size, .. } => image_height(*pixel_size, width, font_size),
        }
    }
}

fn preview_blocks(note: &Note) -> Vec<PreviewBlock> {
    let tags = if note.tags.is_empty() {
        "-".to_owned()
    } else {
        note.tags
            .iter()
            .map(|tag| format!("#{tag}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let mut blocks = vec![PreviewBlock::Text(Text::from(vec![
        Line::from(Span::styled(
            note.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Tags: {tags}")),
        Line::from(""),
    ]))];

    for block in &note.blocks {
        match block {
            NoteBlock::Markdown(markdown) => {
                blocks.push(PreviewBlock::Text(markdown_to_text(markdown)));
                blocks.push(PreviewBlock::Text(Text::from("")));
            }
            NoteBlock::Image {
                href,
                alt,
                resolved,
            } => {
                if let Some(path) = resolved {
                    blocks.push(PreviewBlock::Image {
                        pixel_size: image_dimensions(path).ok(),
                        path: path.clone(),
                        href: href.clone(),
                        alt: alt.clone(),
                    });
                } else {
                    blocks.push(PreviewBlock::Text(Text::from(format!(
                        "[image not found: {alt} -> {href}]"
                    ))));
                }
                blocks.push(PreviewBlock::Text(Text::from("")));
            }
            NoteBlock::Formula(formula) => {
                blocks.push(PreviewBlock::Text(Text::from(vec![
                    Line::from(Span::styled(
                        "[formula]",
                        Style::default().fg(Color::Magenta),
                    )),
                    Line::from(formula.clone()),
                    Line::from(""),
                ])));
            }
        }
    }

    blocks
}

fn render_preview_blocks(
    frame: &mut Frame<'_>,
    images: &mut ImageStore,
    image_hit_areas: &mut Vec<ImageHitArea>,
    preview_scroll: u16,
    area: Rect,
    blocks: &[PreviewBlock],
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let font_size = images.picker.font_size();
    let mut skip = preview_scroll;
    let mut y = area.y;
    let bottom = area.y.saturating_add(area.height);

    for block in blocks {
        let block_height = block.height(area.width, font_size);
        if skip >= block_height {
            skip -= block_height;
            continue;
        }

        match block {
            PreviewBlock::Text(text) => {
                let visible_height = block_height
                    .saturating_sub(skip)
                    .min(bottom.saturating_sub(y));
                if visible_height == 0 {
                    break;
                }

                let block_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: visible_height,
                };
                let paragraph = Paragraph::new(text.clone())
                    .wrap(Wrap { trim: false })
                    .scroll((skip, 0));
                frame.render_widget(paragraph, block_area);
                y = y.saturating_add(visible_height);
            }
            PreviewBlock::Image {
                path, href, alt, ..
            } => {
                if block_height == 0 {
                    continue;
                }

                let available_height = bottom.saturating_sub(y);
                if available_height == 0 {
                    break;
                }
                if y != area.y && block_height > available_height {
                    break;
                }

                let image_width = inline_image_width(area.width).min(area.width);
                let image_height = block_height.min(available_height);
                if image_width == 0 || image_height == 0 {
                    break;
                }

                let block_area = Rect {
                    x: area.x + area.width.saturating_sub(image_width) / 2,
                    y,
                    width: image_width,
                    height: image_height,
                };
                render_image_block(frame, images, block_area, path, href, alt);
                image_hit_areas.push(ImageHitArea {
                    area: block_area,
                    path: path.clone(),
                });
                y = y.saturating_add(image_height);
            }
        }

        skip = 0;
        if y >= bottom {
            break;
        }
    }
}

fn render_fullscreen_image(frame: &mut Frame<'_>, images: &mut ImageStore, path: &Path) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Изображение")
        .title_bottom("Esc: закрыть")
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let href = path.to_string_lossy();
    render_image_block(frame, images, inner, path, href.as_ref(), "image");
}

fn render_image_block(
    frame: &mut Frame<'_>,
    images: &mut ImageStore,
    area: Rect,
    path: &Path,
    href: &str,
    alt: &str,
) {
    if area.height < 2 {
        return;
    }

    match images.cached_image(path) {
        CachedImage::Ready(protocol) => {
            let image = StatefulImage::default().resize(Resize::Scale(None));
            frame.render_stateful_widget(image, area, protocol);
            if let Some(Err(error)) = protocol.last_encoding_result() {
                let fallback = Paragraph::new(format!("[image render error: {alt} -> {error}]"))
                    .style(Style::default().fg(Color::Red))
                    .wrap(Wrap { trim: false });
                frame.render_widget(fallback, area);
            }
        }
        CachedImage::Failed(error) => {
            let fallback = Paragraph::new(format!("[image load error: {alt} -> {href}: {error}]"))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false });
            frame.render_widget(fallback, area);
        }
    }
}

fn image_height(pixel_size: Option<(u32, u32)>, width: u16, font_size: (u16, u16)) -> u16 {
    const FALLBACK_HEIGHT: u16 = 6;
    const MIN_HEIGHT: u64 = 4;

    let Some((pixel_width, pixel_height)) = pixel_size else {
        return FALLBACK_HEIGHT;
    };
    let image_width = inline_image_width(width);
    if pixel_width == 0 || pixel_height == 0 || image_width == 0 || font_size.1 == 0 {
        return FALLBACK_HEIGHT;
    }

    let target_pixel_width = u64::from(image_width) * u64::from(font_size.0.max(1));
    let height = (u64::from(pixel_height) * target_pixel_width)
        .div_ceil(u64::from(pixel_width) * u64::from(font_size.1))
        .clamp(MIN_HEIGHT, u64::from(u16::MAX));

    height as u16
}

fn inline_image_width(width: u16) -> u16 {
    if width == 0 { 0 } else { (width / 2).max(1) }
}

fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::{image_height, inline_image_width, rect_contains};
    use ratatui::layout::Rect;

    #[test]
    fn image_width_uses_half_of_text_width() {
        assert_eq!(inline_image_width(80), 40);
        assert_eq!(inline_image_width(1), 1);
        assert_eq!(inline_image_width(0), 0);
    }

    #[test]
    fn image_height_scales_to_half_text_width() {
        assert_eq!(image_height(Some((1000, 500)), 80, (10, 20)), 10);
    }

    #[test]
    fn image_height_has_reasonable_fallback() {
        assert_eq!(image_height(None, 80, (10, 20)), 6);
        assert_eq!(image_height(Some((0, 500)), 80, (10, 20)), 6);
    }

    #[test]
    fn rect_contains_uses_exclusive_bottom_right_edges() {
        let rect = Rect::new(10, 5, 4, 3);
        assert!(rect_contains(rect, 10, 5));
        assert!(rect_contains(rect, 13, 7));
        assert!(!rect_contains(rect, 14, 7));
        assert!(!rect_contains(rect, 13, 8));
    }
}
