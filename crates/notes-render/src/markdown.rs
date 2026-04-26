use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use unicode_width::UnicodeWidthStr;

pub fn markdown_to_text(markdown: &str) -> Text<'static> {
    let mut renderer = MarkdownRenderer::default();
    let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES);

    for event in parser {
        renderer.push_event(event);
    }

    renderer.finish(markdown)
}

#[derive(Default)]
struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    spans: Vec<Span<'static>>,
    styles: Vec<Style>,
    heading: Option<HeadingLevel>,
    table: Option<TableState>,
}

impl MarkdownRenderer {
    fn push_event(&mut self, event: Event<'_>) {
        if self.table.is_some() {
            self.push_table_event(event);
            return;
        }

        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                self.finish_line();
                self.heading = Some(level);
                self.styles.push(heading_style(level));
            }
            Event::End(TagEnd::Heading(_)) => {
                self.finish_line();
                self.heading = None;
                self.styles.pop();
                self.lines.push(Line::from(""));
            }
            Event::Start(Tag::Table(alignments)) => {
                self.finish_line();
                self.table = Some(TableState::new(alignments));
            }
            Event::Start(Tag::Strong) => self.push_style(Modifier::BOLD),
            Event::End(TagEnd::Strong) => {
                self.styles.pop();
            }
            Event::Start(Tag::Emphasis) => self.push_style(Modifier::ITALIC),
            Event::End(TagEnd::Emphasis) => {
                self.styles.pop();
            }
            Event::Start(Tag::Item) => {
                self.finish_line();
                self.spans.push(Span::raw("- "));
            }
            Event::End(TagEnd::Paragraph | TagEnd::Item) => {
                self.finish_line();
                if self.heading.is_none() {
                    self.lines.push(Line::from(""));
                }
            }
            Event::Text(text) => self
                .spans
                .push(Span::styled(text.to_string(), self.current_style())),
            Event::Code(text) => self.spans.push(Span::styled(
                text.to_string(),
                self.current_style().fg(Color::Yellow),
            )),
            Event::SoftBreak | Event::HardBreak => self.finish_line(),
            _ => {}
        }
    }

    fn push_table_event(&mut self, event: Event<'_>) {
        let mut rendered_table = None;
        if let Some(table) = self.table.as_mut() {
            rendered_table = table.push_event(event);
        }

        if let Some(lines) = rendered_table {
            self.lines.extend(lines);
            self.lines.push(Line::from(""));
            self.table = None;
        }
    }

    fn finish(mut self, fallback: &str) -> Text<'static> {
        self.finish_line();

        while self.lines.last().is_some_and(|line| line.spans.is_empty()) {
            self.lines.pop();
        }

        if self.lines.is_empty() {
            Text::from(fallback.to_owned())
        } else {
            Text::from(self.lines)
        }
    }

    fn push_style(&mut self, modifier: Modifier) {
        let style = self.current_style().add_modifier(modifier);
        self.styles.push(style);
    }

    fn current_style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn finish_line(&mut self) {
        if !self.spans.is_empty() {
            self.lines.push(Line::from(std::mem::take(&mut self.spans)));
        }
    }
}

struct TableState {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    header_rows: usize,
    in_head: bool,
}

impl TableState {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: String::new(),
            header_rows: 0,
            in_head: false,
        }
    }

    fn push_event(&mut self, event: Event<'_>) -> Option<Vec<Line<'static>>> {
        match event {
            Event::Start(Tag::TableRow) => {
                self.current_row.clear();
            }
            Event::Start(Tag::TableHead) => {
                self.in_head = true;
                self.current_row.clear();
            }
            Event::Start(Tag::TableCell) => {
                self.current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                self.current_row
                    .push(self.current_cell.trim().replace('\n', " "));
                self.current_cell.clear();
            }
            Event::End(TagEnd::TableHead) => {
                if !self.current_row.is_empty() {
                    self.rows.push(std::mem::take(&mut self.current_row));
                }
                self.header_rows = self.rows.len();
                self.in_head = false;
            }
            Event::End(TagEnd::TableRow) => {
                if !self.in_head && !self.current_row.is_empty() {
                    self.rows.push(std::mem::take(&mut self.current_row));
                }
            }
            Event::End(TagEnd::Table) => return Some(self.render()),
            Event::Text(text) => self.current_cell.push_str(&text),
            Event::Code(text) => {
                self.current_cell.push('`');
                self.current_cell.push_str(&text);
                self.current_cell.push('`');
            }
            Event::SoftBreak | Event::HardBreak => self.current_cell.push(' '),
            _ => {}
        }

        None
    }

    fn render(&self) -> Vec<Line<'static>> {
        let column_count = self.column_count();
        if column_count == 0 {
            return Vec::new();
        }

        let widths = self.column_widths(column_count);
        let mut lines = Vec::new();
        lines.push(border_line("┌", "┬", "┐", &widths));

        for (index, row) in self.rows.iter().enumerate() {
            let is_header = index < self.header_rows.max(1);
            lines.push(row_line(row, &self.alignments, &widths, is_header));
            if index + 1 == self.header_rows.max(1) {
                lines.push(border_line("├", "┼", "┤", &widths));
            }
        }

        lines.push(border_line("└", "┴", "┘", &widths));
        lines
    }

    fn column_count(&self) -> usize {
        self.rows
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or_default()
            .max(self.alignments.len())
    }

    fn column_widths(&self, column_count: usize) -> Vec<usize> {
        let mut widths = vec![1; column_count];

        for row in &self.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }

        widths
    }
}

fn heading_style(level: HeadingLevel) -> Style {
    let color = match level {
        HeadingLevel::H1 => Color::Cyan,
        HeadingLevel::H2 => Color::Green,
        HeadingLevel::H3 => Color::Yellow,
        _ => Color::White,
    };

    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn border_line(left: &str, separator: &str, right: &str, widths: &[usize]) -> Line<'static> {
    let mut line = String::from(left);
    for (index, width) in widths.iter().enumerate() {
        line.push_str(&"─".repeat(width + 2));
        if index + 1 == widths.len() {
            line.push_str(right);
        } else {
            line.push_str(separator);
        }
    }

    Line::from(Span::styled(line, Style::default().fg(Color::DarkGray)))
}

fn row_line(
    row: &[String],
    alignments: &[Alignment],
    widths: &[usize],
    is_header: bool,
) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    for (index, width) in widths.iter().enumerate() {
        let cell = row.get(index).map(String::as_str).unwrap_or_default();
        let alignment = alignments.get(index).copied().unwrap_or(Alignment::None);
        let style = if is_header {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        spans.push(Span::raw(" "));
        spans.push(Span::styled(align_cell(cell, *width, alignment), style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
    }

    Line::from(spans)
}

fn align_cell(cell: &str, width: usize, alignment: Alignment) -> String {
    let used = UnicodeWidthStr::width(cell);
    let padding = width.saturating_sub(used);

    match alignment {
        Alignment::Right => format!("{}{}", " ".repeat(padding), cell),
        Alignment::Center => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{}{}", " ".repeat(left), cell, " ".repeat(right))
        }
        Alignment::Left | Alignment::None => format!("{}{}", cell, " ".repeat(padding)),
    }
}

#[cfg(test)]
mod tests {
    use super::markdown_to_text;

    fn plain_lines(markdown: &str) -> Vec<String> {
        markdown_to_text(markdown)
            .lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn renders_headings_and_inline_code() {
        let lines = plain_lines("# Title\n\nUse `code` here.");

        assert_eq!(lines[0], "Title");
        assert!(lines.iter().any(|line| line == "Use code here."));
    }

    #[test]
    fn renders_tables() {
        let lines = plain_lines("| Name | Score |\n|------|------:|\n| Алиса | 10 |\n| Bob | 2 |");

        assert!(lines.iter().any(|line| line.contains("Name")));
        assert!(lines.iter().any(|line| line.contains("Алиса")));
        assert!(lines.iter().any(|line| line.contains("│")));
    }
}
