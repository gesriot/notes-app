use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use unicode_width::UnicodeWidthStr;

pub fn markdown_to_text(markdown: &str) -> Text<'static> {
    let mut renderer = MarkdownRenderer::default();
    let markdown = inline_math_to_text(markdown);
    let parser = Parser::new_ext(&markdown, Options::ENABLE_TABLES);

    for event in parser {
        renderer.push_event(event);
    }

    renderer.finish(&markdown)
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
            Event::Text(text) => self.spans.push(Span::styled(
                inline_math_to_text(&text),
                self.current_style(),
            )),
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
            Event::Text(text) => self.current_cell.push_str(&inline_math_to_text(&text)),
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

fn inline_math_to_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("$$") {
            output.push_str("$$");
            index += 2;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("non-empty slice has at least one char");
        if character != '$' || is_escaped(input, index) {
            output.push(character);
            index += character.len_utf8();
            continue;
        }

        let content_start = index + 1;
        let Some(content_end) = find_inline_math_end(input, content_start) else {
            output.push(character);
            index += character.len_utf8();
            continue;
        };

        output.push_str(&math_expression_to_text(
            input[content_start..content_end].trim(),
        ));
        index = content_end + 1;
    }

    output
}

fn find_inline_math_end(input: &str, start: usize) -> Option<usize> {
    for (offset, character) in input[start..].char_indices() {
        let index = start + offset;
        if character == '$' && !is_escaped(input, index) && !input[index..].starts_with("$$") {
            return Some(index);
        }
    }

    None
}

fn is_escaped(input: &str, index: usize) -> bool {
    let mut slash_count = 0usize;
    for byte in input[..index].bytes().rev() {
        if byte == b'\\' {
            slash_count += 1;
        } else {
            break;
        }
    }

    slash_count % 2 == 1
}

fn math_expression_to_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        let rest = &input[index..];
        if let Some((replacement, after_command)) = structured_math_command(input, index) {
            output.push_str(&replacement);
            index = after_command;
            continue;
        }

        let character = rest
            .chars()
            .next()
            .expect("non-empty slice has at least one char");
        if matches!(character, '_' | '^') {
            if let Some((script, after_script)) = read_script(input, index + character.len_utf8()) {
                if character == '_' {
                    output.push_str(&subscript_text(&script));
                } else {
                    output.push_str(&superscript_text(&script));
                }
                index = after_script;
                continue;
            }
        }

        if character == '\\' {
            let (command, after_command) = read_command(input, index);
            if let Some(replacement) = simple_math_command(command) {
                output.push_str(replacement);
            } else if command == "\\" {
                output.push(' ');
            } else {
                output.push_str(command.trim_start_matches('\\'));
            }
            index = after_command;
            continue;
        }

        if !matches!(character, '{' | '}') {
            output.push(character);
        }
        index += character.len_utf8();
    }

    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read_script(input: &str, index: usize) -> Option<(String, usize)> {
    if index >= input.len() {
        return None;
    }

    if input[index..].starts_with('{') {
        let (content, after_group) = braced_group(input, index)?;
        return Some((math_expression_to_text(content), after_group));
    }

    if input[index..].starts_with('\\') {
        if let Some((replacement, after_command)) = structured_math_command(input, index) {
            return Some((replacement, after_command));
        }

        let (command, after_command) = read_command(input, index);
        let replacement = simple_math_command(command)
            .map(str::to_owned)
            .unwrap_or_else(|| command.trim_start_matches('\\').to_owned());
        return Some((replacement, after_command));
    }

    let character = input[index..].chars().next()?;
    Some((character.to_string(), index + character.len_utf8()))
}

fn subscript_text(text: &str) -> String {
    script_text(text, subscript_char, '₍', '₎')
}

fn superscript_text(text: &str) -> String {
    script_text(text, superscript_char, '⁽', '⁾')
}

fn script_text(text: &str, mapper: fn(char) -> Option<char>, left: char, right: char) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join("");
    if compact.is_empty() {
        return String::new();
    }

    if let Some(mapped) = compact.chars().map(mapper).collect::<Option<String>>() {
        return mapped;
    }

    format!("{left}{compact}{right}")
}

fn subscript_char(character: char) -> Option<char> {
    Some(match character {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        _ => return None,
    })
}

fn superscript_char(character: char) -> Option<char> {
    Some(match character {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        _ => return None,
    })
}

fn structured_math_command(input: &str, index: usize) -> Option<(String, usize)> {
    let rest = &input[index..];
    if rest.starts_with(r"\frac") {
        let first_open = index + r"\frac".len();
        let (numerator, after_numerator) = braced_group(input, first_open)?;
        let (denominator, after_denominator) = braced_group(input, after_numerator)?;
        return Some((
            format!(
                "({})/({})",
                math_expression_to_text(numerator),
                math_expression_to_text(denominator)
            ),
            after_denominator,
        ));
    }

    if rest.starts_with(r"\sqrt") {
        let mut group_open = index + r"\sqrt".len();
        let mut root = None;
        if input[group_open..].starts_with('[')
            && let Some((content, after_bracket)) = bracket_group(input, group_open)
        {
            root = Some(content);
            group_open = after_bracket;
        }

        let (content, after_group) = braced_group(input, group_open)?;
        let content = math_expression_to_text(content);
        let replacement = if let Some(root) = root {
            format!("√[{}]({content})", math_expression_to_text(root))
        } else {
            format!("√({content})")
        };
        return Some((replacement, after_group));
    }

    for command in [
        r"\mathbf",
        r"\boldsymbol",
        r"\mathrm",
        r"\mathit",
        r"\operatorname",
    ] {
        if rest.starts_with(command) {
            let (content, after_group) = braced_group(input, index + command.len())?;
            return Some((math_expression_to_text(content), after_group));
        }
    }

    if rest.starts_with(r"\mathbb") {
        let (content, after_group) = braced_group(input, index + r"\mathbb".len())?;
        let content = math_expression_to_text(content)
            .chars()
            .map(double_struck_char)
            .collect::<String>();
        return Some((content, after_group));
    }

    if rest.starts_with(r"\text") {
        let (content, after_group) = braced_group(input, index + r"\text".len())?;
        return Some((content.to_owned(), after_group));
    }

    None
}

fn braced_group(input: &str, open_brace: usize) -> Option<(&str, usize)> {
    delimited_group(input, open_brace, '{', '}')
}

fn bracket_group(input: &str, open_bracket: usize) -> Option<(&str, usize)> {
    delimited_group(input, open_bracket, '[', ']')
}

fn delimited_group(
    input: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<(&str, usize)> {
    if !input[open_index..].starts_with(open) {
        return None;
    }

    let mut depth = 0usize;
    let content_start = open_index + open.len_utf8();

    for (offset, character) in input[open_index..].char_indices() {
        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let close_index = open_index + offset;
                return Some((
                    &input[content_start..close_index],
                    close_index + close.len_utf8(),
                ));
            }
        }
    }

    None
}

fn read_command(input: &str, index: usize) -> (&str, usize) {
    let after_slash = index + 1;
    let mut end = after_slash;

    for (offset, character) in input[after_slash..].char_indices() {
        if character.is_ascii_alphabetic() {
            end = after_slash + offset + character.len_utf8();
        } else {
            break;
        }
    }

    if end == after_slash {
        let character = input[after_slash..]
            .chars()
            .next()
            .expect("command slash has following char");
        end = after_slash + character.len_utf8();
    }

    (&input[index..end], end)
}

fn simple_math_command(command: &str) -> Option<&'static str> {
    Some(match command {
        r"\alpha" => "α",
        r"\beta" => "β",
        r"\gamma" => "γ",
        r"\delta" => "δ",
        r"\epsilon" => "ε",
        r"\lambda" => "λ",
        r"\mu" => "μ",
        r"\theta" => "θ",
        r"\phi" => "φ",
        r"\pi" => "π",
        r"\sigma" => "σ",
        r"\Delta" => "Δ",
        r"\Sigma" | r"\sum" => "Σ",
        r"\times" => "×",
        r"\subseteq" => "⊆",
        r"\in" => "∈",
        r"\cdot" => "·",
        r"\approx" => "≈",
        r"\neq" | r"\ne" => "≠",
        r"\leq" | r"\le" => "≤",
        r"\geq" | r"\ge" => "≥",
        r"\cap" => "∩",
        r"\cup" => "∪",
        r"\emptyset" => "∅",
        r"\to" | r"\rightarrow" | r"\xrightarrow" => "→",
        r"\left" | r"\right" => "",
        r"\log" => "log",
        r"\ln" => "ln",
        r"\sin" => "sin",
        r"\cos" => "cos",
        r"\tan" => "tan",
        r"\quad" | r"\qquad" | r"\," | r"\;" | r"\:" | r"\ " => " ",
        _ => return None,
    })
}

fn double_struck_char(character: char) -> char {
    match character {
        'A' => '𝔸',
        'B' => '𝔹',
        'C' => 'ℂ',
        'D' => '𝔻',
        'E' => '𝔼',
        'F' => '𝔽',
        'G' => '𝔾',
        'H' => 'ℍ',
        'I' => '𝕀',
        'J' => '𝕁',
        'K' => '𝕂',
        'L' => '𝕃',
        'M' => '𝕄',
        'N' => 'ℕ',
        'O' => '𝕆',
        'P' => 'ℙ',
        'Q' => 'ℚ',
        'R' => 'ℝ',
        'S' => '𝕊',
        'T' => '𝕋',
        'U' => '𝕌',
        'V' => '𝕍',
        'W' => '𝕎',
        'X' => '𝕏',
        'Y' => '𝕐',
        'Z' => 'ℤ',
        _ => character,
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

    #[test]
    fn renders_inline_math_as_readable_text() {
        let lines = plain_lines(
            r"Graph $G = (V, E)$, $\mathbf{v}_i \in \mathbb{R}^d$, $\frac{a}{b}$, $\alpha \cdot \beta$.",
        );

        let line = lines.join("\n");
        assert!(line.contains("G = (V, E)"));
        assert!(line.contains("vᵢ ∈ ℝᵈ"));
        assert!(line.contains("(a)/(b)"));
        assert!(line.contains("α · β"));
        assert!(!line.contains('$'));
        assert!(!line.contains(r"\mathbf"));
    }

    #[test]
    fn renders_inline_math_inside_table_cells() {
        let lines = plain_lines(
            r"| Metric | Formula |
|---|---|
| Recall | $\frac{\text{relevant in context}}{\text{total relevant}}$ |
| Space | $\mathbb{R}^d$ |",
        );

        let table = lines.join("\n");
        assert!(table.contains("(relevant in context)/(total relevant)"));
        assert!(table.contains("ℝᵈ"));
        assert!(!table.contains('$'));
    }

    #[test]
    fn renders_inline_math_scripts() {
        let lines =
            plain_lines(r"$e_c$, $K_c$, $A_{ij}$, $E_i^2$, $E_\text{person}$, $\mathbb{R}^d$");

        let line = lines.join("\n");
        assert!(line.contains("e₍c₎"));
        assert!(line.contains("K₍c₎"));
        assert!(line.contains("Aᵢⱼ"));
        assert!(line.contains("Eᵢ²"));
        assert!(line.contains("E₍person₎"));
        assert!(line.contains("ℝᵈ"));
        assert!(!line.contains("e_c"));
        assert!(!line.contains("R^d"));
    }
}
