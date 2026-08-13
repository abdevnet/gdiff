use crate::theme::{TokenColors, TokenStyle};
use eframe::egui::Color32;
use std::path::Path;
use syntect::easy::ScopeRegionIterator;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

pub struct Engine {
    syntaxes: SyntaxSet,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
        }
    }

    pub fn highlight(&self, path: &str, text: &str, tokens: &TokenColors) -> Vec<Vec<Span>> {
        if text.is_empty() {
            return Vec::new();
        }
        let syntax = syntax_for(&self.syntaxes, path);
        let mut parse = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        let mut lines_out = Vec::new();

        for line in LinesWithEndings::from(text) {
            let ops = match parse.parse_line(line, &self.syntaxes) {
                Ok(ops) => ops,
                Err(_) => {
                    lines_out.push(vec![Span::plain(strip_nl(line), tokens.default.color)]);
                    continue;
                }
            };
            let mut spans = Vec::new();
            for (region, op) in ScopeRegionIterator::new(&ops, line) {
                if let Err(_) = stack.apply(op) {
                    continue;
                }
                let region = strip_nl(region);
                if region.is_empty() {
                    continue;
                }
                let style = style_for(&stack, tokens);
                spans.push(Span {
                    text: region.to_string(),
                    color: style.color,
                    italics: style.italic,
                    strong: style.bold,
                });
            }
            lines_out.push(spans);
        }
        lines_out
    }
}

#[derive(Debug, Clone)]
pub struct Span {
    pub text: String,
    pub color: Color32,
    pub italics: bool,
    pub strong: bool,
}

impl Span {
    fn plain(text: &str, color: Color32) -> Self {
        Self {
            text: text.to_string(),
            color,
            italics: false,
            strong: false,
        }
    }
}

fn strip_nl(s: &str) -> &str {
    s.trim_end_matches(['\n', '\r'])
}

fn syntax_for<'a>(ss: &'a SyntaxSet, path: &str) -> &'a SyntaxReference {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    ss.find_syntax_by_extension(ext)
        .or_else(|| {
            let name = match ext {
                "js" | "jsx" => "JavaScript",
                "ts" | "tsx" => "TypeScript",
                "cs" => "C#",
                "rs" => "Rust",
                "py" => "Python",
                "rb" => "Ruby",
                "go" => "Go",
                "java" => "Java",
                "md" => "Markdown",
                "json" => "JSON",
                "yml" | "yaml" => "YAML",
                "toml" => "TOML",
                "sh" | "bash" => "Bourne Again Shell (bash)",
                "ps1" | "psm1" => "PowerShell",
                "html" => "HTML",
                "css" => "CSS",
                "xml" | "csproj" => "XML",
                "sql" => "SQL",
                "c" | "h" => "C",
                "cpp" | "cc" | "cxx" => "C++",
                _ => "",
            };
            if name.is_empty() {
                None
            } else {
                ss.find_syntax_by_name(name)
            }
        })
        .unwrap_or_else(|| ss.find_syntax_plain_text())
}

fn style_for(stack: &ScopeStack, tokens: &TokenColors) -> TokenStyle {
    let mut best = tokens.default;
    for scope in stack.as_slice() {
        let name = scope.build_string();
        if name.contains("comment") {
            best = tokens.comment;
        } else if name.contains("string") {
            best = tokens.string;
        } else if name.contains("keyword") || name.contains("storage") {
            best = tokens.keyword;
        } else if name.contains("constant.numeric")
            || name.contains("constant.language")
            || name.contains("constant.character")
        {
            best = tokens.number;
        } else if name.contains("entity.name.function") {
            best = tokens.function;
        } else if name.contains("entity.name.class")
            || name.contains("entity.name.type")
            || name.contains("entity.name.struct")
            || name.contains("storage.type")
            || name.contains("support.type")
        {
            best = tokens.type_name;
        } else if name.contains("keyword.operator") || name.contains("punctuation") {
            best = tokens.operator;
        }
    }
    best
}
