use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Ident(String),
    Let,
    Print,
    Equals,
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
    Newline,
}

pub fn lex(source: &str) -> Result<Vec<(Token, usize)>, Error> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut pos = 0;
    let mut line = 1;

    while pos < chars.len() {
        let c = chars[pos];

        if c == '/' && chars.get(pos + 1) == Some(&'/') {
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        let single = match c {
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '*' => Some(Token::Star),
            '/' => Some(Token::Slash),
            '(' => Some(Token::LeftParen),
            ')' => Some(Token::RightParen),
            '=' => Some(Token::Equals),
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
                    "print" => Token::Print,
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
}
