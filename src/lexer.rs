use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Ident(String),
    Let,
    Print,
    If,
    Else,
    While,
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
                    "if" => Token::If,
                    "else" => Token::Else,
                    "while" => Token::While,
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
}
