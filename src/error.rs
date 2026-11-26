use std::fmt::Write;
use std::io::IsTerminal;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Error {
    pub line: usize,
    pub message: String,
    pub file: Option<PathBuf>,
}

impl Error {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Error {
            line,
            message: message.into(),
            file: None,
        }
    }

    pub fn render(&self, path: &str, source: &str) -> String {
        let module = self
            .file
            .as_ref()
            .and_then(|file| Some((display(file), std::fs::read_to_string(file).ok()?)));
        let (path, source) = match &module {
            Some((path, source)) => (path.as_str(), source.as_str()),
            None => (path, source),
        };
        let color = std::io::stderr().is_terminal();
        let paint = |code: &str, text: &str| {
            if color {
                format!("\x1b[{code}m{text}\x1b[0m")
            } else {
                text.to_string()
            }
        };

        let mut out = format!(
            "{} {}\n",
            paint("1;31", "error:"),
            paint("1", &self.message)
        );
        let Some(text) = source.lines().nth(self.line.wrapping_sub(1)) else {
            let _ = write!(out, "  --> {path}:{}", self.line);
            return out;
        };
        let number = self.line.to_string();
        let pad = " ".repeat(number.len());
        let (start, width) = underline(text, &self.message);
        let bar = paint("1;34", "|");
        let _ = writeln!(out, "{pad}{} {path}:{}", paint("1;34", "-->"), self.line);
        let _ = writeln!(out, "{pad} {bar}");
        let _ = writeln!(out, "{} {bar} {text}", paint("1;34", &number));
        let marks = format!("{}{}", " ".repeat(start), "^".repeat(width));
        let _ = write!(out, "{pad} {bar} {}", paint("1;31", &marks));
        out
    }
}

fn display(file: &std::path::Path) -> String {
    let here = std::env::current_dir().ok();
    let relative = here.as_ref().and_then(|dir| file.strip_prefix(dir).ok());
    relative.unwrap_or(file).display().to_string()
}

fn quoted(message: &str) -> Option<&str> {
    let after = match message.find("found '") {
        Some(at) => &message[at + 7..],
        None => &message[message.find('\'')? + 1..],
    };
    Some(&after[..after.find('\'')?]).filter(|word| !word.is_empty())
}

fn underline(text: &str, message: &str) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let indent = chars.iter().take_while(|c| c.is_whitespace()).count();
    let whole = (indent, chars.len().saturating_sub(indent).max(1));
    if message.contains("end of the line") || message.contains("end of input") {
        return (chars.len(), 1);
    }
    let Some(word) = quoted(message) else {
        return whole;
    };
    let target: Vec<char> = word.chars().collect();
    let is_name = |c: &char| c.is_alphanumeric() || *c == '_';
    let wordy = target.iter().all(is_name);
    (0..chars.len())
        .find(|&at| {
            chars[at..].starts_with(&target)
                && (!wordy
                    || ((at == 0 || !is_name(&chars[at - 1]))
                        && chars.get(at + target.len()).is_none_or(|c| !is_name(c))))
        })
        .map_or(whole, |at| (at, target.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_at_the_quoted_name() {
        let error = Error::new(2, "'total' should be number, got string");
        let shown = error.render("main.kp", "let a = 1\nlet total: number = \"x\"\n");
        assert_eq!(
            shown,
            "error: 'total' should be number, got string\n \
             --> main.kp:2\n  |\n2 | let total: number = \"x\"\n  |     ^^^^^"
        );
    }

    #[test]
    fn prefers_what_was_found() {
        let (start, width) = underline("let x = (1 + 2", "expected ')', found 'x'");
        assert_eq!((start, width), (4, 1));
    }

    #[test]
    fn matches_whole_words_only() {
        assert_eq!(underline("let ab = a", "'a' is not defined"), (9, 1));
    }

    #[test]
    fn points_past_a_line_that_stops_early() {
        assert_eq!(
            underline("let x = (1", "expected ')', found the end of the line"),
            (10, 1)
        );
    }

    #[test]
    fn underlines_the_line_without_a_name() {
        assert_eq!(underline("    print(1 / 0)", "division by zero"), (4, 12));
    }
}
