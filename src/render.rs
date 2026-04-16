use colored::Colorize;
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::{SyntaxReference, SyntaxSet},
    util::as_24_bit_terminal_escaped,
};

use std::sync::LazyLock;

// Syntax highlighting resources for Markdown code fences
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

pub fn render_markdown_to_terminal(input: &str) -> String {
    use std::fmt::Write as _;
    let ps: &SyntaxSet = &SYNTAX_SET;
    let ts: &ThemeSet = &THEME_SET;
    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .unwrap_or_else(|| ts.themes.values().next().expect("has theme"));

    let mut out = String::new();
    let mut in_code = false;
    let mut highlighter: Option<HighlightLines> = None;

    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("```") {
            if in_code {
                // closing fence
                in_code = false;
                highlighter = None;
            } else {
                // opening fence; parse language
                let l = rest.trim();
                let syntax: &SyntaxReference = (!l.is_empty()).then_some(l).map_or_else(
                    || ps.find_syntax_plain_text(),
                    |token| {
                        ps.find_syntax_by_token(token)
                            .or_else(|| ps.find_syntax_by_name(token))
                            .or_else(|| ps.find_syntax_by_extension(token))
                            .unwrap_or_else(|| ps.find_syntax_plain_text())
                    },
                );
                highlighter = Some(HighlightLines::new(syntax, theme));
                in_code = true;
                // draw a subtle fence line
                let fence = "```".truecolor(120, 120, 120).to_string();
                let _ = writeln!(out, "{fence}");
            }
            continue;
        }

        // both branches end here; avoid branches_sharing_code lint by moving shared code here if any

        if in_code {
            if let Some(ref mut h) = highlighter {
                let line_with_nl = format!("{line}\n");
                let ranges = h.highlight_line(&line_with_nl, ps).unwrap_or_else(|_| {
                    vec![(
                        syntect::highlighting::Style::default(),
                        line_with_nl.as_str(),
                    )]
                });
                let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                out.push_str(&escaped);
            } else {
                out.push_str(&(line.to_string() + "\n"));
            }
        } else {
            // Non-code text: keep subtle color
            out.push_str(&line.truecolor(220, 220, 220).to_string());
            out.push('\n');
        }
    }
    out
}
