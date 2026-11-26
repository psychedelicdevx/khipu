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
}

#[derive(Debug)]
pub enum LexError {
    Unexpected(char),
    BadNumber(String),
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LexError::Unexpected(c) => write!(f, "unexpected character '{c}'"),
            LexError::BadNumber(text) => write!(f, "'{text}' is not a valid number"),
        }
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut pos = 0;

    while pos < chars.len() {
        let c = chars[pos];

        match c {
            ' ' | '\t' => pos += 1,
            '+' => {
                tokens.push(Token::Plus);
                pos += 1
            }
            '-' => {
                tokens.push(Token::Minus);
                pos += 1
            }
            '*' => {
                tokens.push(Token::Star);
                pos += 1
            }
            '/' => {
                tokens.push(Token::Slash);
                pos += 1
            }
            '(' => {
                tokens.push(Token::LeftParen);
                pos += 1
            }
            ')' => {
                tokens.push(Token::RightParen);
                pos += 1
            }
            '=' => {
                tokens.push(Token::Equals);
                pos += 1
            }
            c if c.is_ascii_digit() => {
                let start = pos;
                while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
                    pos += 1;
                }
                let text: String = chars[start..pos].iter().collect();
                match text.parse::<f64>() {
                    Ok(value) => tokens.push(Token::Number(value)),
                    Err(_) => return Err(LexError::BadNumber(text)),
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
                tokens.push(token);
            }
            other => return Err(LexError::Unexpected(other)),
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_simple_math() {
        let tokens = lex("2 + 3.5").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Number(2.0), Token::Plus, Token::Number(3.5)]
        );
    }

    #[test]
    fn lexes_keywords_and_names() {
        let tokens = lex("let total_2 = x").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("total_2".to_string()),
                Token::Equals,
                Token::Ident("x".to_string()),
            ]
        );
    }
}
