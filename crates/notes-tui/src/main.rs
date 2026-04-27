use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use image::{ImageReader, image_dimensions};
use notes_render::{
    formula::{normalize_formula, render_formula},
    markdown::markdown_to_text,
};
use notes_vault::{Note, NoteBlock, Vault};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
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
    vault: Vault,
    notes: Vec<Note>,
    filtered_indices: Vec<usize>,
    list_state: ListState,
    focus: Focus,
    search_query: String,
    search_area: Rect,
    preview_scroll: u16,
    list_items_area: Rect,
    image_hit_areas: Vec<ImageHitArea>,
    focused_image: Option<usize>,
    fullscreen_image: Option<PathBuf>,
    images: ImageStore,
    preview_cache: HashMap<PathBuf, Vec<PreviewBlock>>,
    notification: Option<Notification>,
    should_quit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Search,
    List,
    Preview,
}

struct Notification {
    message: String,
    expires_at: Instant,
}

struct ImageHitArea {
    area: Rect,
    path: PathBuf,
}

struct ImageStore {
    picker: Picker,
    cache: HashMap<PathBuf, CachedImage>,
    formula_cache: HashMap<String, CachedFormula>,
    formula_cache_dir: PathBuf,
}

enum CachedImage {
    Ready(StatefulProtocol),
    Failed(String),
}

struct CachedFormula {
    image: CachedImage,
    pixel_size: Option<(u32, u32)>,
}

impl App {
    fn new(vault_root: PathBuf) -> Result<Self> {
        let vault = Vault::new(vault_root);
        let notes = vault
            .load_notes()
            .with_context(|| format!("failed to load vault at {}", vault.root.display()))?;

        let image_picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let formula_cache_dir = vault.root.join(".cache").join("formula");

        let mut app = Self {
            vault,
            notes,
            filtered_indices: Vec::new(),
            list_state: ListState::default(),
            focus: Focus::List,
            search_query: String::new(),
            search_area: Rect::default(),
            preview_scroll: 0,
            list_items_area: Rect::default(),
            image_hit_areas: Vec::new(),
            focused_image: None,
            fullscreen_image: None,
            images: ImageStore::new(image_picker, formula_cache_dir),
            preview_cache: HashMap::new(),
            notification: None,
            should_quit: false,
        };
        app.apply_filter(None);

        Ok(app)
    }

    fn selected_note_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|index| self.filtered_indices.get(index).copied())
    }

    fn next_note(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected();
        let next = current
            .map(|i| (i + 1).min(self.filtered_indices.len() - 1))
            .unwrap_or(0);
        self.list_state.select(Some(next));
        if current != Some(next) {
            self.preview_scroll = 0;
            self.focused_image = None;
        }
    }

    fn previous_note(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected();
        let prev = current.map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(prev));
        if current != Some(prev) {
            self.preview_scroll = 0;
            self.focused_image = None;
        }
    }

    fn page_notes_down(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let current = self.list_state.selected().unwrap_or(0);
        let step = self.list_items_area.height.max(1) as usize;
        self.select_note((current + step).min(self.filtered_indices.len() - 1));
    }

    fn page_notes_up(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let current = self.list_state.selected().unwrap_or(0);
        let step = self.list_items_area.height.max(1) as usize;
        self.select_note(current.saturating_sub(step));
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
        if index >= self.filtered_indices.len() {
            return;
        }

        let current = self.list_state.selected();
        self.list_state.select(Some(index));
        if current != Some(index) {
            self.preview_scroll = 0;
            self.focused_image = None;
        }
    }

    fn focus_search(&mut self) {
        self.focus = Focus::Search;
    }

    fn focus_list(&mut self) {
        self.focus = Focus::List;
    }

    fn focus_preview(&mut self) {
        self.focus = Focus::Preview;
    }

    fn push_search_char(&mut self, character: char) {
        self.search_query.push(character);
        self.apply_filter(None);
    }

    fn pop_search_char(&mut self) {
        if self.search_query.pop().is_some() {
            self.apply_filter(None);
        }
    }

    fn clear_search(&mut self) {
        if !self.search_query.is_empty() {
            self.search_query.clear();
            self.apply_filter(None);
        }
    }

    fn reload_notes(&mut self) -> Result<()> {
        let selected_path = self
            .selected_note_index()
            .map(|index| self.notes[index].path.clone());
        self.notes = self
            .vault
            .load_notes()
            .with_context(|| format!("failed to reload vault at {}", self.vault.root.display()))?;
        self.preview_cache.clear();
        self.images.cache.clear();
        self.apply_filter(selected_path.as_deref());
        self.notify(format!("Заметки перезагружены: {}", self.notes.len()));
        Ok(())
    }

    fn apply_filter(&mut self, preferred_path: Option<&Path>) {
        let tokens = search_tokens(&self.search_query);
        self.filtered_indices = filter_note_indices(&self.notes, &tokens);

        let selected = preferred_path
            .and_then(|path| {
                self.filtered_indices
                    .iter()
                    .position(|note_index| self.notes[*note_index].path == path)
            })
            .or_else(|| (!self.filtered_indices.is_empty()).then_some(0));

        self.list_state = ListState::default();
        self.list_state.select(selected);
        self.preview_scroll = 0;
        self.focused_image = None;
    }

    fn notify(&mut self, message: impl Into<String>) {
        self.notification = Some(Notification {
            message: message.into(),
            expires_at: Instant::now() + Duration::from_millis(1800),
        });
    }

    fn selected_image_paths(&self) -> Vec<PathBuf> {
        self.selected_note_index()
            .and_then(|index| self.notes.get(index))
            .map(|note| {
                note.images
                    .iter()
                    .filter_map(|image| image.resolved.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn cycle_focused_image(&mut self, forward: bool) {
        let images = self.selected_image_paths();
        if images.is_empty() {
            self.focused_image = None;
            self.notify("В текущей заметке нет картинок");
            return;
        }

        let current = self.focused_image.filter(|index| *index < images.len());
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % images.len(),
            (Some(0), false) | (None, false) => images.len() - 1,
            (Some(index), false) => index - 1,
            (None, true) => 0,
        };
        self.focused_image = Some(next);
        self.focus_preview();
        self.notify(format!(
            "Картинка {}/{}: {}",
            next + 1,
            images.len(),
            images[next]
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("image")
        ));
    }

    fn open_focused_image(&mut self) -> Result<()> {
        let images = self.selected_image_paths();
        let Some(path) = self
            .focused_image
            .and_then(|index| images.get(index))
            .or_else(|| images.first())
            .cloned()
        else {
            self.notify("В текущей заметке нет картинок");
            return Ok(());
        };

        opener::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        self.notify(format!(
            "Открыта картинка: {}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("image")
        ));
        Ok(())
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.fullscreen_image.is_some() {
                    return;
                }

                if rect_contains(self.search_area, mouse.column, mouse.row) {
                    self.focus_search();
                    return;
                }

                if let Some(path) = self.image_path_at(mouse.column, mouse.row) {
                    self.focus_preview();
                    self.fullscreen_image = Some(path);
                    return;
                }

                if let Some(index) = self.note_index_at(mouse.column, mouse.row) {
                    self.focus_list();
                    self.select_note(index);
                }
            }
            MouseEventKind::ScrollDown if self.fullscreen_image.is_none() => {
                self.focus_preview();
                self.scroll_preview_by(3)
            }
            MouseEventKind::ScrollUp if self.fullscreen_image.is_none() => {
                self.focus_preview();
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
        (index < self.filtered_indices.len()).then_some(index)
    }
}

impl ImageStore {
    fn new(picker: Picker, formula_cache_dir: PathBuf) -> Self {
        Self {
            picker,
            cache: HashMap::new(),
            formula_cache: HashMap::new(),
            formula_cache_dir,
        }
    }

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

    fn cached_formula(&mut self, formula: &str) -> &mut CachedFormula {
        let normalized = normalize_formula(formula);
        if !self.formula_cache.contains_key(&normalized) {
            let cached = self.load_or_render_formula(formula, &normalized);
            self.formula_cache.insert(normalized.clone(), cached);
        }

        self.formula_cache
            .get_mut(&normalized)
            .expect("formula cache entry inserted before lookup")
    }

    fn load_or_render_formula(&mut self, formula: &str, normalized: &str) -> CachedFormula {
        let cache_path = self.formula_cache_path(normalized);
        if let Ok(png) = fs::read(&cache_path)
            && let Ok(cached) = self.cached_formula_from_png(&png, None)
        {
            return cached;
        }

        match render_formula(formula) {
            Ok(rendered) => {
                let cached = self.cached_formula_from_png(
                    &rendered.png,
                    Some((rendered.width, rendered.height)),
                );
                match cached {
                    Ok(cached) => {
                        let _ = write_formula_cache(&cache_path, &rendered.png);
                        cached
                    }
                    Err(error) => CachedFormula {
                        image: CachedImage::Failed(error),
                        pixel_size: None,
                    },
                }
            }
            Err(error) => CachedFormula {
                image: CachedImage::Failed(error.to_string()),
                pixel_size: None,
            },
        }
    }

    fn cached_formula_from_png(
        &mut self,
        png: &[u8],
        pixel_size: Option<(u32, u32)>,
    ) -> Result<CachedFormula, String> {
        let image = image::load_from_memory(png).map_err(|error| error.to_string())?;
        let pixel_size = pixel_size.or(Some((image.width(), image.height())));

        Ok(CachedFormula {
            image: CachedImage::Ready(self.picker.new_resize_protocol(image)),
            pixel_size,
        })
    }

    fn formula_cache_path(&self, normalized: &str) -> PathBuf {
        self.formula_cache_dir
            .join(format!("{}.png", sha1_hex(normalized)))
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
                    if let Err(error) = handle_key(&mut app, key) {
                        app.notify(format!("{error:#}"));
                    }
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        app.should_quit = true;
        return Ok(());
    }

    if app.fullscreen_image.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.close_fullscreen_image();
            }
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                app.should_quit = true;
            }
            _ => {}
        }
        return Ok(());
    }

    if app.focus == Focus::Search {
        return handle_search_key(app, key);
    }

    handle_navigation_key(app, key)
}

fn handle_search_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Esc | KeyCode::Enter, _) => app.focus_list(),
        (KeyCode::Backspace, _) => app.pop_search_char(),
        (KeyCode::Char('u'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_search()
        }
        (KeyCode::Char('l'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus_list()
        }
        (KeyCode::Char('r'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.reload_notes()?
        }
        (KeyCode::Char('o'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_focused_image()?
        }
        (KeyCode::Down, _) => app.next_note(),
        (KeyCode::Up, _) => app.previous_note(),
        (KeyCode::Char(character), modifiers)
            if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT =>
        {
            app.push_search_char(character);
        }
        _ => {}
    }

    Ok(())
}

fn handle_navigation_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), modifiers) if modifiers.is_empty() => {
            app.should_quit = true;
        }
        (KeyCode::Esc, _) => app.should_quit = true,
        (KeyCode::Char('/'), modifiers) if modifiers.is_empty() => app.focus_search(),
        (KeyCode::Char('l'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus_list()
        }
        (KeyCode::Char('r'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.reload_notes()?
        }
        (KeyCode::Char('o'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_focused_image()?
        }
        (KeyCode::Down, modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.cycle_focused_image(true)
        }
        (KeyCode::Up, modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.cycle_focused_image(false)
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            app.focus_list();
            app.next_note();
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
            app.focus_list();
            app.previous_note();
        }
        (KeyCode::PageDown, _) if app.focus == Focus::List => app.page_notes_down(),
        (KeyCode::PageUp, _) if app.focus == Focus::List => app.page_notes_up(),
        (KeyCode::PageDown, _) | (KeyCode::Char(' '), _) => {
            app.focus_preview();
            app.scroll_preview_down();
        }
        (KeyCode::PageUp, _) | (KeyCode::Char('b'), _) => {
            app.focus_preview();
            app.scroll_preview_up();
        }
        (KeyCode::Char('d'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus_preview();
            app.scroll_preview_down();
        }
        (KeyCode::Char('u'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus_preview();
            app.scroll_preview_up();
        }
        _ => {}
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
    render_search(frame, app, vertical[1]);
    frame.render_widget(footer(app), vertical[3]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(20)])
        .split(vertical[2]);

    render_notes(frame, app, main[0]);
    render_preview(frame, app, main[1]);

    if let Some(path) = app.fullscreen_image.clone() {
        render_fullscreen_image(frame, &mut app.images, &path);
    }

    render_notification(frame, app);
}

fn header() -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled("Vault Notes", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" - поиск и просмотр заметок"),
    ]))
}

fn render_search(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let is_focused = app.focus == Focus::Search;
    let title = if is_focused {
        "Поиск по тегам"
    } else {
        "Поиск"
    };
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);
    let inner = block.inner(area);
    app.search_area = area;
    let query = if app.search_query.is_empty() {
        Span::styled(
            " / чтобы искать по тегам",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::raw(app.search_query.clone())
    };

    frame.render_widget(Paragraph::new(Line::from(query)).block(block), area);

    if is_focused && inner.width > 0 && inner.height > 0 {
        let cursor_offset = app
            .search_query
            .chars()
            .count()
            .min(inner.width.saturating_sub(1) as usize) as u16;
        frame.set_cursor_position(Position::new(inner.x + cursor_offset, inner.y));
    }
}

fn footer(app: &App) -> Paragraph<'static> {
    Paragraph::new(format!(
        "{}  /:поиск  Ctrl+L:список  Ctrl+R:reload  Ctrl+↑/↓:картинки  Ctrl+O:open  Space/b:preview  q:выход",
        focus_label(app.focus)
    ))
}

fn focus_label(focus: Focus) -> &'static str {
    match focus {
        Focus::Search => "Фокус: поиск",
        Focus::List => "Фокус: список",
        Focus::Preview => "Фокус: preview",
    }
}

fn render_notification(frame: &mut Frame<'_>, app: &mut App) {
    let Some(notification) = app.notification.as_ref() else {
        return;
    };
    if Instant::now() >= notification.expires_at {
        app.notification = None;
        return;
    }

    let area = frame.area();
    if area.width < 8 || area.height < 3 {
        return;
    }

    let message = notification.message.clone();
    let width =
        (message.chars().count() as u16 + 4).clamp(12, area.width.saturating_sub(2).max(12));
    let height = 3;
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height + 1),
        width: width.min(area.width),
        height,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(message)
            .block(block)
            .style(Style::default().fg(Color::Yellow)),
        popup,
    );
}

fn render_notes(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let items = app
        .filtered_indices
        .iter()
        .map(|index| ListItem::new(app.notes[*index].title.clone()))
        .collect::<Vec<_>>();

    let block = Block::default().borders(Borders::ALL).title(format!(
        "Заметки {}/{}",
        app.filtered_indices.len(),
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

    let Some(index) = app.selected_note_index() else {
        let message = if app.notes.is_empty() {
            "Vault пуст или папка notes/ не найдена"
        } else {
            "Нет заметок по текущему фильтру"
        };
        frame.render_widget(Paragraph::new(message), inner);
        return;
    };
    let note_key = app.notes[index].path.clone();
    if !app.preview_cache.contains_key(&note_key) {
        let blocks = preview_blocks(&app.notes[index], &mut app.images);
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
    Formula {
        formula: String,
        pixel_size: Option<(u32, u32)>,
    },
}

impl PreviewBlock {
    fn height(&self, width: u16, font_size: (u16, u16)) -> u16 {
        match self {
            Self::Text(text) => text.lines.len().max(1) as u16,
            Self::Image { pixel_size, .. } => image_height(*pixel_size, width, font_size),
            Self::Formula { pixel_size, .. } => formula_height(*pixel_size, width, font_size),
        }
    }
}

fn preview_blocks(note: &Note, images: &mut ImageStore) -> Vec<PreviewBlock> {
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
                // Эагерный рендер: получаем pixel_size сразу, чтобы layout был стабилен
                // на первом фрейме и не дёргался после кэширования.
                let pixel_size = images.cached_formula(formula).pixel_size;
                blocks.push(PreviewBlock::Formula {
                    formula: formula.clone(),
                    pixel_size,
                });
                blocks.push(PreviewBlock::Text(Text::from("")));
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
            PreviewBlock::Formula { formula, .. } => {
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

                let image_width = formula_image_width(area.width).min(area.width);
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
                render_formula_block(frame, images, block_area, formula);
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

fn render_formula_block(frame: &mut Frame<'_>, images: &mut ImageStore, area: Rect, formula: &str) {
    if area.height < 2 {
        return;
    }

    match &mut images.cached_formula(formula).image {
        CachedImage::Ready(protocol) => {
            let image = StatefulImage::default().resize(Resize::Scale(None));
            frame.render_stateful_widget(image, area, protocol);
            if let Some(Err(error)) = protocol.last_encoding_result() {
                let fallback =
                    Paragraph::new(format!("[formula render error: {error}]\n{formula}"))
                        .style(Style::default().fg(Color::Red))
                        .wrap(Wrap { trim: false });
                frame.render_widget(fallback, area);
            }
        }
        CachedImage::Failed(error) => {
            let fallback = Paragraph::new(format!("[formula error: {error}]\n{formula}"))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false });
            frame.render_widget(fallback, area);
        }
    }
}

fn image_height(pixel_size: Option<(u32, u32)>, width: u16, font_size: (u16, u16)) -> u16 {
    const FALLBACK_HEIGHT: u16 = 6;
    scaled_image_height(
        pixel_size,
        inline_image_width(width),
        font_size,
        FALLBACK_HEIGHT,
    )
}

fn formula_height(pixel_size: Option<(u32, u32)>, width: u16, font_size: (u16, u16)) -> u16 {
    const FALLBACK_HEIGHT: u16 = 4;
    scaled_image_height(
        pixel_size,
        formula_image_width(width),
        font_size,
        FALLBACK_HEIGHT,
    )
}

fn scaled_image_height(
    pixel_size: Option<(u32, u32)>,
    cell_width: u16,
    font_size: (u16, u16),
    fallback_height: u16,
) -> u16 {
    const MIN_HEIGHT: u64 = 4;

    let Some((pixel_width, pixel_height)) = pixel_size else {
        return fallback_height;
    };
    if pixel_width == 0 || pixel_height == 0 || cell_width == 0 || font_size.1 == 0 {
        return fallback_height;
    }

    let target_pixel_width = u64::from(cell_width) * u64::from(font_size.0.max(1));
    let height = (u64::from(pixel_height) * target_pixel_width)
        .div_ceil(u64::from(pixel_width) * u64::from(font_size.1))
        .clamp(MIN_HEIGHT, u64::from(u16::MAX));

    height as u16
}

fn inline_image_width(width: u16) -> u16 {
    if width == 0 { 0 } else { (width / 2).max(1) }
}

fn formula_image_width(width: u16) -> u16 {
    width
}

fn write_formula_cache(path: &Path, png: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, png)
}

fn sha1_hex(value: &str) -> String {
    format!("{:x}", Sha1::digest(value.as_bytes()))
}

fn search_tokens(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|token| token.trim_start_matches('#').to_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn filter_note_indices(notes: &[Note], tokens: &[String]) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| note_matches_tokens(note, tokens).then_some(index))
        .collect()
}

fn note_matches_tokens(note: &Note, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return true;
    }

    tokens.iter().all(|token| {
        note.tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(token))
    })
}

fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::{
        filter_note_indices, formula_height, image_height, inline_image_width, rect_contains,
        search_tokens,
    };
    use notes_vault::Note;
    use ratatui::layout::Rect;
    use std::path::PathBuf;

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
    fn formula_height_uses_full_text_width() {
        assert_eq!(formula_height(Some((1000, 500)), 80, (10, 20)), 20);
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

    #[test]
    fn search_tokens_strip_hashes_and_empty_parts() {
        assert_eq!(
            search_tokens("  #GraphRAG  rust  "),
            vec!["graphrag".to_owned(), "rust".to_owned()]
        );
    }

    #[test]
    fn filters_notes_by_tag_tokens_with_and_semantics() {
        let notes = vec![
            note("a.md", &["graphrag", "rust"]),
            note("b.md", &["graphrag"]),
            note("c.md", &["rust"]),
        ];
        let tokens = search_tokens("graph rust");

        assert_eq!(filter_note_indices(&notes, &tokens), [0]);
    }

    fn note(path: &str, tags: &[&str]) -> Note {
        Note {
            path: PathBuf::from(path),
            title: path.to_owned(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            blocks: Vec::new(),
            images: Vec::new(),
        }
    }
}
