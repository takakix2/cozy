use ratatui::style::{Style, Color};
use regex::Regex;

/// Simple syntax highlighting using regex patterns
pub struct SyntaxHighlighter {
    patterns: Vec<(Regex, Style)>,
}

impl SyntaxHighlighter {
    pub fn new(language: Option<&str>) -> Self {
        let patterns = match language {
            Some("rust") | Some("rs") => rust_patterns(),
            Some("python") | Some("py") => python_patterns(),
            Some("javascript") | Some("js") => javascript_patterns(),
            Some("json") => json_patterns(),
            Some("toml") => toml_patterns(),
            _ => vec![], // No highlighting for unknown languages
        };
        
        Self { patterns }
    }

    /// Highlight a line of text
    pub fn highlight(&self, line: &str) -> Vec<(String, Style)> {
        if self.patterns.is_empty() {
            return vec![(line.to_string(), Style::default())];
        }

        let mut result = Vec::new();
        let mut last_end = 0;
        let mut matches: Vec<(usize, usize, Style)> = Vec::new();

        // Find all matches
        for (pattern, style) in &self.patterns {
            for mat in pattern.find_iter(line) {
                matches.push((mat.start(), mat.end(), *style));
            }
        }

        // Sort by start position
        matches.sort_by_key(|m| m.0);

        // Merge overlapping matches (later patterns take precedence)
        let mut merged = Vec::new();
        for (start, end, style) in matches {
            if let Some((last_start, last_end, _)) = merged.last_mut() {
                if start < *last_end {
                    // Overlapping, replace
                    *last_start = start;
                    *last_end = end.max(*last_end);
                } else {
                    merged.push((start, end, style));
                }
            } else {
                merged.push((start, end, style));
            }
        }

        // Build result
        for (start, end, style) in merged {
            if start > last_end {
                // Add plain text before match
                result.push((line[last_end..start].to_string(), Style::default()));
            }
            result.push((line[start..end].to_string(), style));
            last_end = end;
        }

        // Add remaining text
        if last_end < line.len() {
            result.push((line[last_end..].to_string(), Style::default()));
        }

        if result.is_empty() {
            result.push((line.to_string(), Style::default()));
        }

        result
    }
}

fn rust_patterns() -> Vec<(Regex, Style)> {
    vec![
        // Keywords
        (Regex::new(r"\b(fn|let|mut|const|pub|struct|enum|impl|trait|use|mod|match|if|else|for|while|loop|return|break|continue|async|await|move|ref|self|Self|super|dyn|unsafe|extern|crate)\b").unwrap(),
         Style::default().fg(Color::Magenta)),
        // Strings
        (Regex::new(r#""([^"\\]|\\.)*""#).unwrap(),
         Style::default().fg(Color::Green)),
        // Comments
        (Regex::new(r"//.*").unwrap(),
         Style::default().fg(Color::DarkGray)),
        (Regex::new(r"/\*.*?\*/").unwrap(),
         Style::default().fg(Color::DarkGray)),
        // Numbers
        (Regex::new(r"\b\d+\b").unwrap(),
         Style::default().fg(Color::Yellow)),
        // Types
        (Regex::new(r"\b(i8|i16|i32|i64|i128|u8|u16|u32|u64|u128|usize|isize|f32|f64|bool|char|str|String|Vec|Option|Result)\b").unwrap(),
         Style::default().fg(Color::Cyan)),
    ]
}

fn python_patterns() -> Vec<(Regex, Style)> {
    vec![
        // Keywords
        (Regex::new(r"\b(def|class|if|elif|else|for|while|try|except|finally|with|as|import|from|return|yield|break|continue|pass|raise|assert|lambda|None|True|False|and|or|not|in|is|del|global|nonlocal)\b").unwrap(),
         Style::default().fg(Color::Magenta)),
        // Strings
        (Regex::new(r#""([^"\\]|\\.)*""#).unwrap(),
         Style::default().fg(Color::Green)),
        (Regex::new(r"'([^'\\]|\\.)*'").unwrap(),
         Style::default().fg(Color::Green)),
        // Comments
        (Regex::new(r"#.*").unwrap(),
         Style::default().fg(Color::DarkGray)),
        // Numbers
        (Regex::new(r"\b\d+\.?\d*\b").unwrap(),
         Style::default().fg(Color::Yellow)),
    ]
}

fn javascript_patterns() -> Vec<(Regex, Style)> {
    vec![
        // Keywords
        (Regex::new(r"\b(function|const|let|var|class|extends|if|else|for|while|try|catch|finally|return|yield|break|continue|switch|case|default|async|await|import|export|from|as|new|this|super|typeof|instanceof|in|of|true|false|null|undefined)\b").unwrap(),
         Style::default().fg(Color::Magenta)),
        // Strings
        (Regex::new(r#""([^"\\]|\\.)*""#).unwrap(),
         Style::default().fg(Color::Green)),
        (Regex::new(r"'([^'\\]|\\.)*'").unwrap(),
         Style::default().fg(Color::Green)),
        (Regex::new(r"`([^`\\]|\\.)*`").unwrap(),
         Style::default().fg(Color::Green)),
        // Comments
        (Regex::new(r"//.*").unwrap(),
         Style::default().fg(Color::DarkGray)),
        (Regex::new(r"/\*.*?\*/").unwrap(),
         Style::default().fg(Color::DarkGray)),
        // Numbers
        (Regex::new(r"\b\d+\.?\d*\b").unwrap(),
         Style::default().fg(Color::Yellow)),
    ]
}

fn json_patterns() -> Vec<(Regex, Style)> {
    vec![
        // Keys
        (Regex::new(r#""([^"]+)":\s*"#).unwrap(),
         Style::default().fg(Color::Cyan)),
        // Strings
        (Regex::new(r#""([^"\\]|\\.)*""#).unwrap(),
         Style::default().fg(Color::Green)),
        // Numbers
        (Regex::new(r"\b\d+\.?\d*\b").unwrap(),
         Style::default().fg(Color::Yellow)),
        // Booleans and null
        (Regex::new(r"\b(true|false|null)\b").unwrap(),
         Style::default().fg(Color::Magenta)),
    ]
}

fn toml_patterns() -> Vec<(Regex, Style)> {
    vec![
        // Keys
        (Regex::new(r"^\[.*\]$").unwrap(),
         Style::default().fg(Color::Cyan)),
        (Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*\s*=").unwrap(),
         Style::default().fg(Color::Yellow)),
        // Strings
        (Regex::new(r#""([^"\\]|\\.)*""#).unwrap(),
         Style::default().fg(Color::Green)),
        (Regex::new(r"'([^'\\]|\\.)*'").unwrap(),
         Style::default().fg(Color::Green)),
        // Numbers
        (Regex::new(r"\b\d+\.?\d*\b").unwrap(),
         Style::default().fg(Color::Yellow)),
        // Booleans
        (Regex::new(r"\b(true|false)\b").unwrap(),
         Style::default().fg(Color::Magenta)),
        // Comments
        (Regex::new(r"#.*").unwrap(),
         Style::default().fg(Color::DarkGray)),
    ]
}
