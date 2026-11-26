use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Text(String),
    Code(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Str(String),
    Template(Vec<StringPart>),
    Ident(String),
    Let,
    Fn,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    True,
    False,
    Equals,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Bang,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Newline,
}

pub fn lex(source: &str) -> Result<Vec<(Token, usize)>, Error> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut pos = 0;
    let mut line = 1;

    while pos < chars.len() {
        let c = chars[pos];
        let next = chars.get(pos + 1).copied();

        if c == '/' && next == Some('/') {
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        let double = match (c, next) {
            ('=', Some('=')) => Some(Token::EqualEqual),
            ('!', Some('=')) => Some(Token::BangEqual),
            ('<', Some('=')) => Some(Token::LessEqual),
            ('>', Some('=')) => Some(Token::GreaterEqual),
            ('&', Some('&')) => Some(Token::AndAnd),
            ('|', Some('|')) => Some(Token::OrOr),
            _ => None,
        };
        if let Some(token) = double {
            tokens.push((token, line));
            pos += 2;
            continue;
        }

        let single = match c {
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '*' => Some(Token::Star),
            '/' => Some(Token::Slash),
            '%' => Some(Token::Percent),
            '(' => Some(Token::LeftParen),
            ')' => Some(Token::RightParen),
            '{' => Some(Token::LeftBrace),
            '}' => Some(Token::RightBrace),
            '[' => Some(Token::LeftBracket),
            ']' => Some(Token::RightBracket),
            ',' => Some(Token::Comma),
            '=' => Some(Token::Equals),
            '<' => Some(Token::Less),
            '>' => Some(Token::Greater),
            '!' => Some(Token::Bang),
            '\n' => Some(Token::Newline),
            _ => None,
        };
        if let Some(token) = single {
            tokens.push((token, line));
            if c == '\n' {
                line += 1;
            }
            pos += 1;
            continue;
        }

        match c {
            ' ' | '\t' | '\r' => pos += 1,
            '"' => {
                let (token, end) = lex_string(&chars, pos, line)?;
                tokens.push((token, line));
                pos = end;
            }
            c if c.is_ascii_digit() => {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
                    pos += 1;
                }
                let text: String = chars[start..pos].iter().collect();
                match text.parse::<f64>() {
                    Ok(value) => tokens.push((Token::Number(value), line)),
                    Err(_) => {
                        return Err(Error {
                            line,
                            message: format!("'{text}' is not a valid number"),
                        });
                    }
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                    pos += 1;
                }
                let word: String = chars[start..pos].iter().collect();
                let token = match word.as_str() {
                    "let" => Token::Let,
                    "fn" => Token::Fn,
                    "return" => Token::Return,
                    "if" => Token::If,
                    "else" => Token::Else,
                    "while" => Token::While,
                    "for" => Token::For,
                    "in" => Token::In,
                    "break" => Token::Break,
                    "continue" => Token::Continue,
                    "true" => Token::True,
                    "false" => Token::False,
                    _ => Token::Ident(word),
                };
                tokens.push((token, line));
            }
            other => {
                return Err(Error {
                    line,
                    message: format!("unexpected character '{other}'"),
                });
            }
        }
    }

    Ok(tokens)
}

fn lex_string(chars: &[char], start: usize, line: usize) -> Result<(Token, usize), Error> {
    let unterminated = |what: &str| Error {
        line,
        message: format!("{what} is never closed"),
    };
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut pos = start + 1;

    loop {
        match chars.get(pos) {
            None | Some('\n') => return Err(unterminated("string")),
            Some('"') => {
                pos += 1;
                break;
            }
            Some('\\') => {
                let escaped = match chars.get(pos + 1) {
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('"') => '"',
                    Some('\\') => '\\',
                    Some('$') => '$',
                    Some(other) => {
                        return Err(Error {
                            line,
                            message: format!("unknown escape '\\{other}'"),
                        });
                    }
                    None => return Err(unterminated("string")),
                };
                text.push(escaped);
                pos += 2;
            }
            Some('$') if chars.get(pos + 1) == Some(&'{') => {
                parts.push(StringPart::Text(std::mem::take(&mut text)));
                pos += 2;
                let code_start = pos;
                let mut depth = 1;
                while depth > 0 {
                    match chars.get(pos) {
                        None | Some('\n') => return Err(unterminated("'${'")),
                        Some('{') => depth += 1,
                        Some('}') => depth -= 1,
                        _ => {}
                    }
                    pos += 1;
                }
                parts.push(StringPart::Code(
                    chars[code_start..pos - 1].iter().collect(),
                ));
            }
            Some(&c) => {
                text.push(c);
                pos += 1;
            }
        }
    }

    if parts.is_empty() {
        Ok((Token::Str(text), pos))
    } else {
        parts.push(StringPart::Text(text));
        Ok((Token::Template(parts), pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(source: &str) -> Vec<Token> {
        lex(source)
            .unwrap()
            .into_iter()
            .map(|(token, _)| token)
            .collect()
    }

    #[test]
    fn lexes_simple_math() {
        assert_eq!(
            tokens("2 + 3.5"),
            vec![Token::Number(2.0), Token::Plus, Token::Number(3.5)]
        );
    }

    #[test]
    fn lexes_keywords_and_names() {
        assert_eq!(
            tokens("let total_2 = x"),
            vec![
                Token::Let,
                Token::Ident("total_2".to_string()),
                Token::Equals,
                Token::Ident("x".to_string()),
            ]
        );
    }

    #[test]
    fn skips_comments_and_counts_lines() {
        let result = lex("// hello\nx // the x\ny").unwrap();
        assert_eq!(
            result,
            vec![
                (Token::Newline, 1),
                (Token::Ident("x".to_string()), 2),
                (Token::Newline, 2),
                (Token::Ident("y".to_string()), 3),
            ]
        );
    }

    #[test]
    fn lexes_two_character_operators() {
        assert_eq!(
            tokens("a <= b == !c && d != e || f >= g"),
            vec![
                Token::Ident("a".to_string()),
                Token::LessEqual,
                Token::Ident("b".to_string()),
                Token::EqualEqual,
                Token::Bang,
                Token::Ident("c".to_string()),
                Token::AndAnd,
                Token::Ident("d".to_string()),
                Token::BangEqual,
                Token::Ident("e".to_string()),
                Token::OrOr,
                Token::Ident("f".to_string()),
                Token::GreaterEqual,
                Token::Ident("g".to_string()),
            ]
        );
    }

    #[test]
    fn lexes_strings_with_escapes() {
        assert_eq!(
            tokens(r#""say \"hi\"\n" "\${x}""#),
            vec![
                Token::Str("say \"hi\"\n".to_string()),
                Token::Str("${x}".to_string()),
            ]
        );
    }

    #[test]
    fn splits_interpolated_strings() {
        assert_eq!(
            tokens(r#""total: ${price + tax}!""#),
            vec![Token::Template(vec![
                StringPart::Text("total: ".to_string()),
                StringPart::Code("price + tax".to_string()),
                StringPart::Text("!".to_string()),
            ])]
        );
    }

    #[test]
    fn reports_unclosed_strings() {
        let error = lex("let a = 1\nlet b = \"oops").unwrap_err();
        assert_eq!(error.line, 2);
        assert_eq!(error.message, "string is never closed");
    }

    #[test]
    fn lexes_lists_and_loops() {
        assert_eq!(
            tokens("for x in [1, 2] { break }"),
            vec![
                Token::For,
                Token::Ident("x".to_string()),
                Token::In,
                Token::LeftBracket,
                Token::Number(1.0),
                Token::Comma,
                Token::Number(2.0),
                Token::RightBracket,
                Token::LeftBrace,
                Token::Break,
                Token::RightBrace,
            ]
        );
    }
}
