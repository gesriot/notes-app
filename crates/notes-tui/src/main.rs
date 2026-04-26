use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use notes_render::markdown::markdown_to_text;
use notes_vault::{Note, NoteBlock, Vault};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{path::PathBuf, time::Duration};

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
    should_quit: bool,
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

        Ok(Self {
            notes,
            list_state,
            preview_scroll: 0,
            should_quit: false,
        })
    }

    fn selected_note(&self) -> Option<&Note> {
        self.list_state.selected().and_then(|i| self.notes.get(i))
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
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, App::new(args.vault)?);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| draw(frame, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), modifiers)
                            if modifiers.is_empty()
                                || modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.should_quit = true;
                        }
                        (KeyCode::Esc, _) => app.should_quit = true,
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.next_note(),
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.previous_note(),
                        (KeyCode::PageDown, _) => app.scroll_preview_down(),
                        (KeyCode::PageUp, _) => app.scroll_preview_up(),
                        _ => {}
                    }
                }
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
    Paragraph::new("↑/↓ или j/k: выбор  PgUp/PgDn: preview  q/Ctrl+Q/Esc: выход")
}

fn render_notes(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let items = app
        .notes
        .iter()
        .map(|note| ListItem::new(note.title.clone()))
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Заметки {}/{}",
            app.notes.len(),
            app.notes.len()
        )))
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

fn render_preview(frame: &mut Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let preview = app
        .selected_note()
        .map(preview_text)
        .unwrap_or_else(|| Text::from("Vault пуст или папка notes/ не найдена"));
    let visible_height = area.height.saturating_sub(2) as usize;
    let max_scroll = preview.lines.len().saturating_sub(visible_height) as u16;
    app.preview_scroll = app.preview_scroll.min(max_scroll);

    let paragraph = Paragraph::new(preview)
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));

    frame.render_widget(paragraph, area);
}

fn preview_text(note: &Note) -> Text<'static> {
    let tags = if note.tags.is_empty() {
        "-".to_owned()
    } else {
        note.tags
            .iter()
            .map(|tag| format!("#{tag}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let mut lines = vec![
        Line::from(Span::styled(
            note.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Tags: {tags}")),
        Line::from(""),
    ];

    for block in &note.blocks {
        match block {
            NoteBlock::Markdown(markdown) => {
                lines.extend(markdown_to_text(markdown).lines);
                lines.push(Line::from(""));
            }
            NoteBlock::Image { href, alt, .. } => {
                lines.push(Line::from(format!("[image: {alt} -> {href}]")));
                lines.push(Line::from(""));
            }
            NoteBlock::Formula(formula) => {
                lines.push(Line::from("[formula]"));
                lines.push(Line::from(formula.clone()));
                lines.push(Line::from(""));
            }
        }
    }

    Text::from(lines)
}
