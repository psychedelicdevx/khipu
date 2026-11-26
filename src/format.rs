use crate::error::Error;
use crate::lexer::{Token, scan};

const INDENT: &str = "    ";

pub fn format(source: &str) -> Result<String, Error> {
    let mut lines: Vec<String> = Vec::new();
    let mut depth: usize = 0;

    for (number, raw) in source.lines().enumerate() {
        let chars: Vec<char> = raw.chars().collect();
        let (tokens, spans) = scan(raw).map_err(|error| Error::new(number + 1, error.message))?;
        let tokens: Vec<Token> = tokens.into_iter().map(|(token, _)| token).collect();
        let code_end = spans.last().map_or(0, |span| span.1);
        let rest: String = chars[code_end..].iter().collect();
        let comment = rest.trim();

        if tokens.is_empty() && comment.is_empty() {
            lines.push(String::new());
            continue;
        }

        let closing = tokens.iter().take_while(|token| closes(token)).count();
        let clause = matches!(tokens.first(), Some(Token::Ident(word)) if matches!(word.as_str(), "requires" | "ensures" | "example"))
            && !matches!(
                tokens.get(1),
                None | Some(
                    Token::Equals
                        | Token::Dot
                        | Token::QuestionDot
                        | Token::LeftParen
                        | Token::LeftBracket
                )
            );
        let indent = INDENT.repeat(depth.saturating_sub(closing) + usize::from(clause));
        let mut line = indent + &join(&tokens, &spans, &chars);
        if !comment.is_empty() {
            if !tokens.is_empty() {
                line.push(' ');
            }
            line.push_str(comment);
        }
        lines.push(line);

        for token in &tokens {
            if opens(token) {
                depth += 1;
            } else if closes(token) {
                depth = depth.saturating_sub(1);
            }
        }
    }

    let mut out = String::new();
    let mut blank = false;
    for line in &lines {
        if line.is_empty() {
            blank = true;
            continue;
        }
        let after_open = out.trim_end().ends_with(['{', '[', '(']);
        let before_close = line.trim_start().starts_with(['}', ']', ')']);
        if blank && !out.is_empty() && !after_open && !before_close {
            out.push('\n');
        }
        blank = false;
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

fn opens(token: &Token) -> bool {
    matches!(
        token,
        Token::LeftBrace | Token::LeftBracket | Token::LeftParen
    )
}

fn closes(token: &Token) -> bool {
    matches!(
        token,
        Token::RightBrace | Token::RightBracket | Token::RightParen
    )
}

fn is_value_end(token: &Token) -> bool {
    matches!(
        token,
        Token::Number(_)
            | Token::Str(_)
            | Token::Template(_)
            | Token::Ident(_)
            | Token::True
            | Token::False
            | Token::NoneKeyword
            | Token::RightParen
            | Token::RightBracket
            | Token::RightBrace
            | Token::Question
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Plain,
    Unary,
    GenericOpen,
    GenericClose,
}

fn roles(tokens: &[Token], spans: &[(usize, usize)]) -> Vec<Role> {
    let mut roles = vec![Role::Plain; tokens.len()];
    let mut generic = 0;
    for (index, token) in tokens.iter().enumerate() {
        let prev = index.checked_sub(1).map(|at| &tokens[at]);
        let tight = index > 0 && spans[index - 1].1 == spans[index].0;
        roles[index] = match token {
            Token::Minus | Token::Bang if !prev.is_some_and(is_value_end) => Role::Unary,
            Token::Bang => Role::Unary,
            Token::Less if tight && matches!(prev, Some(Token::Ident(_))) => {
                generic += 1;
                Role::GenericOpen
            }
            Token::Greater if generic > 0 => {
                generic -= 1;
                Role::GenericClose
            }
            _ => Role::Plain,
        };
    }
    roles
}

pub fn tokens_text(tokens: &[Token]) -> String {
    let spans: Vec<(usize, usize)> = (0..tokens.len()).map(|at| (2 * at, 2 * at + 1)).collect();
    let roles = roles(tokens, &spans);
    let mut out = String::new();
    for (index, token) in tokens.iter().enumerate() {
        if index > 0 && spaced(&tokens[index - 1], roles[index - 1], token, roles[index]) {
            out.push(' ');
        }
        match token {
            Token::Ident(name) => out.push_str(name),
            Token::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => {
                out.push_str(&(*n as i64).to_string())
            }
            Token::Number(n) => out.push_str(&n.to_string()),
            Token::Str(text) => out.push_str(&format!("{text:?}")),
            Token::Template(parts) => {
                out.push('"');
                for part in parts {
                    match part {
                        crate::lexer::StringPart::Text(text) => out.push_str(text),
                        crate::lexer::StringPart::Code(code) => {
                            out.push_str("${");
                            out.push_str(code);
                            out.push('}');
                        }
                    }
                }
                out.push('"');
            }
            Token::Newline => {}
            other => out.push_str(other.to_string().trim_matches('\'')),
        }
    }
    out
}

fn join(tokens: &[Token], spans: &[(usize, usize)], chars: &[char]) -> String {
    let roles = roles(tokens, spans);
    let mut out = String::new();
    for (index, token) in tokens.iter().enumerate() {
        if index > 0 && spaced(&tokens[index - 1], roles[index - 1], token, roles[index]) {
            out.push(' ');
        }
        let (start, end) = spans[index];
        out.extend(&chars[start..end]);
    }
    out
}

fn spaced(prev: &Token, prev_role: Role, next: &Token, next_role: Role) -> bool {
    use Token::*;
    if matches!(prev_role, Role::Unary | Role::GenericOpen)
        || matches!(next_role, Role::GenericOpen | Role::GenericClose)
    {
        return false;
    }
    if matches!(
        next,
        RightParen | RightBracket | Comma | Colon | Dot | QuestionDot | Question
    ) || matches!(prev, LeftParen | LeftBracket | Dot | QuestionDot)
    {
        return false;
    }
    if matches!((prev, next), (LeftBrace, RightBrace)) {
        return false;
    }
    let callable = matches!(prev, Ident(_) | RightParen | RightBracket | Fn)
        || prev_role == Role::GenericClose;
    if matches!(next, LeftParen) && callable {
        return false;
    }
    let indexable = matches!(prev, Ident(_) | RightParen | RightBracket | Str(_));
    !(matches!(next, LeftBracket) && indexable)
}

#[cfg(test)]
mod tests {
    use super::format;

    fn same(source: &str) {
        assert_eq!(format(source).unwrap(), source);
    }

    #[test]
    fn spaces_operators_and_tightens_brackets() {
        assert_eq!(
            format("let  x=( 1+2 )*-3\nprint( x ,[1,2][ 0 ] )\n").unwrap(),
            "let x = (1 + 2) * -3\nprint(x, [1, 2][0])\n"
        );
    }

    #[test]
    fn reindents_blocks_and_keeps_comments() {
        assert_eq!(
            format("fn f(a: number) -> number {\n// double it\n  return a*2 // done\n}\n").unwrap(),
            "fn f(a: number) -> number {\n    // double it\n    return a * 2 // done\n}\n"
        );
    }

    #[test]
    fn keeps_generics_types_and_records_tidy() {
        same("fn largest<T>(items: list<T>, score: fn(T) -> number) -> T? {\n    return none\n}\n");
        same("let p = Point { x: 1, y: -2 }\nlet empty = {}\n");
        same("if !done && a < b {\n    print(\"${a} < ${b}\")\n}\n");
    }

    #[test]
    fn indents_contracts() {
        assert_eq!(
            format("fn half(n: number) -> number\nrequires n >= 0\n  example half(4) == 2\n{\nreturn n / 2\n}\nlet example = 1\n").unwrap(),
            "fn half(n: number) -> number\n    requires n >= 0\n    example half(4) == 2\n{\n    return n / 2\n}\nlet example = 1\n"
        );
    }

    #[test]
    fn collapses_blank_lines() {
        assert_eq!(
            format("\n\nlet a = 1\n\n\n\nlet b = 2\n{\n\nlet c = 3\n\n}\n\n").unwrap(),
            "let a = 1\n\nlet b = 2\n{\n    let c = 3\n}\n"
        );
    }

    #[test]
    fn formatting_twice_changes_nothing() {
        for entry in std::fs::read_dir("examples").unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "kp") {
                let once = format(&std::fs::read_to_string(&path).unwrap()).unwrap();
                assert_eq!(format(&once).unwrap(), once, "{}", path.display());
            }
        }
    }
}
