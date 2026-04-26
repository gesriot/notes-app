use mathjax_svg_rs::Options;
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};
use std::{thread, time::Duration};
use thiserror::Error;

const DEFAULT_FONT_SIZE: f64 = 18.0;
const DEFAULT_SCALE: f32 = 2.0;
const SVG_COLOR: &str = "#ffffff";

#[derive(Debug, Error)]
pub enum FormulaError {
    #[error("MathJax failed: {0}")]
    MathJax(String),
    #[error("SVG parse failed: {0}")]
    Svg(String),
    #[error("failed to allocate formula pixmap")]
    PixmapAllocation,
    #[error("PNG encode failed: {0}")]
    Png(String),
}

#[derive(Debug, Clone)]
pub struct RenderedFormula {
    pub svg: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn render_formula(formula: &str) -> Result<RenderedFormula, FormulaError> {
    render_formula_with_options(formula, DEFAULT_FONT_SIZE, DEFAULT_SCALE)
}

pub fn render_formula_with_options(
    formula: &str,
    font_size: f64,
    scale: f32,
) -> Result<RenderedFormula, FormulaError> {
    let svg = render_formula_svg(formula, font_size)?;
    let (png, width, height) = render_svg_to_png(&svg, scale)?;

    Ok(RenderedFormula {
        svg,
        png,
        width,
        height,
    })
}

pub fn render_formula_svg(formula: &str, font_size: f64) -> Result<String, FormulaError> {
    let formula = normalize_formula(formula);
    let options = Options {
        font_size,
        ..Options::default()
    };

    let svg = render_tex_with_retry(&formula, &options).map_err(FormulaError::MathJax)?;
    Ok(tint_svg(&svg, SVG_COLOR))
}

fn render_tex_with_retry(formula: &str, options: &Options) -> Result<String, String> {
    const MAX_ATTEMPTS: usize = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        match mathjax_svg_rs::render_tex(formula, options) {
            Ok(svg) => return Ok(svg),
            Err(error) if is_mathjax_retry(&error) && attempt < MAX_ATTEMPTS => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("render_tex_with_retry always returns from the loop")
}

fn is_mathjax_retry(error: &str) -> bool {
    error.contains("MathJax retry")
}

fn render_svg_to_png(svg: &str, scale: f32) -> Result<(Vec<u8>, u32, u32), FormulaError> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|error| FormulaError::Svg(error.to_string()))?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or(FormulaError::PixmapAllocation)?;

    let mut pixmap =
        Pixmap::new(size.width(), size.height()).ok_or(FormulaError::PixmapAllocation)?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let png = pixmap
        .encode_png()
        .map_err(|error| FormulaError::Png(error.to_string()))?;

    Ok((png, size.width(), size.height()))
}

pub fn normalize_formula(formula: &str) -> String {
    let compact = formula.split_whitespace().collect::<Vec<_>>().join(" ");
    let compact = expand_mathbb_commands(&compact);
    let mut normalized = String::with_capacity(compact.len());

    for character in compact.chars() {
        push_mathjax_safe_char(&mut normalized, character);
    }

    normalized
}

fn expand_mathbb_commands(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with(r"\mathbb") {
            let open_brace = index + r"\mathbb".len();
            if let Some((content, after_group)) = braced_group(input, open_brace) {
                output.push_str(&mathbb_to_tex_safe(content));
                index = after_group;
                continue;
            }
        }

        let character = rest
            .chars()
            .next()
            .expect("non-empty slice has at least one char");
        output.push(character);
        index += character.len_utf8();
    }

    output
}

fn braced_group(input: &str, open_brace: usize) -> Option<(&str, usize)> {
    if !input[open_brace..].starts_with('{') {
        return None;
    }

    let mut depth = 0usize;
    let content_start = open_brace + 1;

    for (offset, character) in input[open_brace..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let close_brace = open_brace + offset;
                    return Some((&input[content_start..close_brace], close_brace + 1));
                }
            }
            _ => {}
        }
    }

    None
}

fn mathbb_to_tex_safe(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut characters = content.chars().peekable();

    while let Some(character) = characters.next() {
        if character == '\\' {
            output.push(character);
            while let Some(next) = characters.peek().copied() {
                if next.is_ascii_alphabetic() {
                    output.push(next);
                    characters.next();
                } else {
                    break;
                }
            }
            continue;
        }

        output.push_str(&mathbb_char_to_tex_safe(character));
    }

    output
}

fn mathbb_char_to_tex_safe(character: char) -> String {
    match character {
        'R' => r"\mathrm{I\!R}".to_owned(),
        'N' => r"\mathrm{I\!N}".to_owned(),
        'Z' => r"\mathrm{Z\!\!Z}".to_owned(),
        'C' => r"\mathrm{I\!C}".to_owned(),
        'Q' => r"\mathrm{I\!Q}".to_owned(),
        'A'..='Z' | 'a'..='z' | '0'..='9' => format!(r"\mathrm{{{character}}}"),
        _ => character.to_string(),
    }
}

fn tint_svg(svg: &str, color: &str) -> String {
    svg.replacen("<svg ", &format!(r#"<svg color="{color}" "#), 1)
}

fn push_mathjax_safe_char(output: &mut String, character: char) {
    match character {
        '<' => {
            output.push_str(r"\lt ");
            return;
        }
        '>' => {
            output.push_str(r"\gt ");
            return;
        }
        _ => {}
    }

    output.push(character);
}

#[cfg(test)]
mod tests {
    use super::{normalize_formula, render_formula, render_formula_svg};
    use std::{fs, path::Path};

    const REPRESENTATIVE_FORMULAS: &[&str] = &[
        r"Q = \frac{1}{2m} \sum_{ij} \left[ A_{ij} - \frac{k_i k_j}{2m} \right] \delta(c_i, c_j)",
        r"\text{Knowledge Graph Types:} \begin{cases} \text{Entity Graph} & \text{- core entities} \\ \text{Relation Graph} & \text{- entity relations} \end{cases}",
        r"\text{Entities} \xrightarrow{\text{Louvain/Leiden}} \text{Communities} \xrightarrow{\text{LLM}} \text{Structured Reports}",
        r"\text{Cost}_\text{index} = \underbrace{O(n \cdot t_\text{LLM})}_{\text{extraction}} + \underbrace{O(|V| \cdot t_\text{LLM})}_{\text{entity summarization}}",
        r"\mathbf{q} = \text{embed}(\text{Query}) \in \mathbb{R}^{1536}",
        r"\text{Score} = \sqrt[3]{\text{Relevance} \times \text{Completeness} \times \text{Consistency}}",
    ];

    #[test]
    fn normalizes_formula_whitespace() {
        assert_eq!(normalize_formula("  a  \n + \t b  "), "a + b");
        assert_eq!(
            normalize_formula(r"\mathbf{q} \in \mathbb{R}^{1536}"),
            r"\mathbf{q} \in \mathrm{I\!R}^{1536}"
        );
        assert_eq!(
            normalize_formula(r"x < \theta \quad y > 0"),
            r"x \lt  \theta \quad y \gt  0"
        );
    }

    #[test]
    fn preserves_common_math_styles_for_mathjax() {
        let normalized = normalize_formula(r"\mathbf{v}_i \in \mathbb{R}^d");

        assert_eq!(normalized, r"\mathbf{v}_i \in \mathrm{I\!R}^d");
        assert!(normalized.contains(r"\mathbf"));
        assert!(normalized.contains(r"\mathrm{I\!R}"));
        assert!(!normalized.contains(r"\mathbb"));
    }

    #[test]
    fn renders_representative_formula_to_svg() {
        let svg = render_formula_svg(REPRESENTATIVE_FORMULAS[0], 18.0).unwrap();

        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("<path"));
        assert!(svg.contains("color=\"#ffffff\""));
    }

    #[test]
    fn renders_representative_formulas_to_png() {
        for formula in REPRESENTATIVE_FORMULAS {
            let rendered = render_formula(formula)
                .unwrap_or_else(|error| panic!("failed to render {formula}: {error}"));

            assert!(rendered.width > 0);
            assert!(rendered.height > 0);
            assert!(rendered.svg.starts_with("<svg "));
            assert!(rendered.png.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }

    #[test]
    #[ignore = "spike test: renders every display formula from the sample vault"]
    fn renders_current_vault_formulas_to_png() {
        let formulas = current_vault_formulas();
        assert!(formulas.len() > 40);

        for formula in formulas {
            let rendered = render_formula(&formula)
                .unwrap_or_else(|error| panic!("failed to render {formula}: {error}"));

            assert!(rendered.width > 0);
            assert!(rendered.height > 0);
            assert!(rendered.png.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }

    fn current_vault_formulas() -> Vec<String> {
        let notes_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vault/notes");
        let mut formulas = Vec::new();

        for entry in fs::read_dir(notes_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
                continue;
            }

            let content = fs::read_to_string(path).unwrap();
            let mut rest = content.as_str();
            while let Some(start) = rest.find("$$") {
                let after_start = &rest[start + 2..];
                let Some(end) = after_start.find("$$") else {
                    break;
                };

                formulas.push(after_start[..end].trim().to_owned());
                rest = &after_start[end + 2..];
            }
        }

        formulas
    }
}
