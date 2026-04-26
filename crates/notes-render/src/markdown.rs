use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

pub fn markdown_to_text(markdown: &str) -> Text<'static> {
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut styles = vec![Style::default()];
    let mut heading = None;

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                finish_line(&mut lines, &mut spans);
                heading = Some(level);
                styles.push(heading_style(level));
            }
            Event::End(TagEnd::Heading(_)) => {
                finish_line(&mut lines, &mut spans);
                heading = None;
                styles.pop();
                lines.push(Line::from(""));
            }
            Event::Start(Tag::Strong) => push_style(&mut styles, Modifier::BOLD),
            Event::End(TagEnd::Strong) => {
                styles.pop();
            }
            Event::Start(Tag::Emphasis) => push_style(&mut styles, Modifier::ITALIC),
            Event::End(TagEnd::Emphasis) => {
                styles.pop();
            }
            Event::Start(Tag::Item) => spans.push(Span::raw("- ")),
            Event::End(TagEnd::Paragraph | TagEnd::Item) => {
                finish_line(&mut lines, &mut spans);
                if heading.is_none() {
                    lines.push(Line::from(""));
                }
            }
            Event::Text(text) => spans.push(Span::styled(text.to_string(), current_style(&styles))),
            Event::Code(text) => spans.push(Span::styled(
                text.to_string(),
                current_style(&styles).fg(Color::Yellow),
            )),
            Event::SoftBreak | Event::HardBreak => finish_line(&mut lines, &mut spans),
            _ => {}
        }
    }

    finish_line(&mut lines, &mut spans);

    while lines.last().is_some_and(|line| line.spans.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        Text::from(markdown.to_owned())
    } else {
        Text::from(lines)
    }
}

fn push_style(styles: &mut Vec<Style>, modifier: Modifier) {
    let style = current_style(styles).add_modifier(modifier);
    styles.push(style);
}

fn current_style(styles: &[Style]) -> Style {
    styles.last().copied().unwrap_or_default()
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

fn finish_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(std::mem::take(spans)));
    }
}
