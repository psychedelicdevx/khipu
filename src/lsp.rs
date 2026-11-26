use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use serde_json::{Value as Json, json};

use crate::checker::Checker;
use crate::error::{Error, underline};
use crate::format::format;
use crate::lexer::lex;
use crate::parser::{Parser, Type};

const KEYWORDS: [&str; 20] = [
    "let", "fn", "return", "if", "else", "while", "for", "in", "break", "continue", "import",
    "from", "struct", "enum", "match", "true", "false", "none", "test", "select",
];

struct Document {
    text: String,
    error: Option<Error>,
    latest: Checker,
    good: Option<Checker>,
}

impl Document {
    fn new(text: String) -> Self {
        let mut checker = Checker::new();
        let error = lex(&text)
            .and_then(|tokens| Parser::new(tokens).parse_program())
            .and_then(|program| checker.check(&program))
            .err();
        Document {
            text,
            error,
            latest: checker,
            good: None,
        }
    }

    fn update(&mut self, text: String) {
        let next = Document::new(text);
        let good = match (&next.error, self.good.take()) {
            (None, _) => None,
            (Some(_), good) => good.or_else(|| {
                self.error
                    .is_none()
                    .then(|| std::mem::take(&mut self.latest))
            }),
        };
        *self = Document { good, ..next };
    }

    fn checker(&self) -> &Checker {
        match (&self.error, &self.good) {
            (Some(_), Some(good)) => good,
            _ => &self.latest,
        }
    }

    fn diagnostics(&self) -> Json {
        let Some(error) = &self.error else {
            return json!([]);
        };
        let line = error.line.saturating_sub(1);
        let text = self.text.lines().nth(line).unwrap_or("");
        let (start, width) = underline(text, &error.message);
        json!([{
            "range": {
                "start": { "line": line, "character": start },
                "end": { "line": line, "character": start + width },
            },
            "severity": 1,
            "source": "khipu",
            "message": error.message,
        }])
    }

    fn type_of(&self, name: &str, line: usize) -> Option<Type> {
        let checker = self.checker();
        let mut best: Option<&Type> = None;
        for (declared, found, ty) in &checker.names {
            if found == name && (best.is_none() || *declared <= line + 1) {
                best = Some(ty);
            }
        }
        best.or_else(|| checker.global(name)).cloned()
    }

    fn word_at(&self, line: usize, column: usize) -> Option<(String, Option<String>)> {
        let chars: Vec<char> = self.text.lines().nth(line)?.chars().collect();
        let is_name = |c: &char| c.is_alphanumeric() || *c == '_';
        let mut start = column.min(chars.len());
        while start > 0 && is_name(&chars[start - 1]) {
            start -= 1;
        }
        let mut end = column.min(chars.len());
        while end < chars.len() && is_name(&chars[end]) {
            end += 1;
        }
        let word: String = chars[start..end].iter().collect();
        let receiver = (start > 0 && chars[start - 1] == '.').then(|| {
            let mut from = start - 1;
            while from > 0 && is_name(&chars[from - 1]) {
                from -= 1;
            }
            chars[from..start - 1].iter().collect::<String>()
        });
        Some((word, receiver))
    }

    fn members_of(&self, receiver: &str, line: usize) -> Vec<(String, Type)> {
        self.type_of(receiver, line)
            .map_or(Vec::new(), |ty| self.checker().members(&ty))
    }

    fn hover(&self, line: usize, column: usize) -> Option<String> {
        let (word, receiver) = self.word_at(line, column)?;
        if word.is_empty() {
            return None;
        }
        let ty = match &receiver {
            Some(receiver) => self
                .members_of(receiver, line)
                .into_iter()
                .find(|(name, _)| *name == word)
                .map(|(_, ty)| ty)?,
            None => self.type_of(&word, line)?,
        };
        let body = match &ty {
            Type::StructDef(name) | Type::EnumDef(name) => {
                let kind = if matches!(ty, Type::StructDef(_)) {
                    Type::Struct(name.clone())
                } else {
                    ty.clone()
                };
                let keyword = if matches!(ty, Type::StructDef(_)) {
                    "struct"
                } else {
                    "enum"
                };
                let members: Vec<String> = self
                    .checker()
                    .members(&kind)
                    .into_iter()
                    .map(|(name, ty)| format!("    {name}: {ty}"))
                    .collect();
                format!("{keyword} {name} {{\n{}\n}}", members.join("\n"))
            }
            _ => format!("{word}: {ty}"),
        };
        Some(format!("```khipu\n{body}\n```"))
    }

    fn formatted(&self) -> Option<Json> {
        let formatted = format(&self.text).ok()?;
        let lines = self.text.lines().count() + 1;
        Some(json!([{
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": lines, "character": 0 },
            },
            "newText": formatted,
        }]))
    }

    fn completions(&self, line: usize, column: usize) -> Vec<Json> {
        let Some((_, receiver)) = self.word_at(line, column) else {
            return Vec::new();
        };
        if let Some(receiver) = receiver {
            return self
                .members_of(&receiver, line)
                .into_iter()
                .map(|(name, ty)| {
                    let kind = if matches!(ty, Type::Function(..)) {
                        2
                    } else {
                        5
                    };
                    json!({ "label": name, "kind": kind, "detail": ty.to_string() })
                })
                .collect();
        }
        let checker = self.checker();
        let mut seen: HashMap<&str, &Type> = HashMap::new();
        for (name, ty) in checker.globals() {
            seen.insert(name, ty);
        }
        for (declared, name, ty) in &checker.names {
            if *declared <= line + 1 || !seen.contains_key(name.as_str()) {
                seen.insert(name, ty);
            }
        }
        let mut items: Vec<Json> = seen
            .into_iter()
            .map(|(name, ty)| {
                let kind = match ty {
                    Type::Function(..) => 3,
                    Type::Any if checker.global(name).is_some() => 3,
                    Type::StructDef(_) => 22,
                    Type::EnumDef(_) => 13,
                    _ => 6,
                };
                json!({ "label": name, "kind": kind, "detail": ty.to_string() })
            })
            .collect();
        items.extend(
            KEYWORDS
                .iter()
                .map(|word| json!({ "label": word, "kind": 14 })),
        );
        items
    }
}

fn read(input: &mut impl BufRead) -> Option<Json> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn send(out: &mut impl Write, message: Json) {
    let body = message.to_string();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{body}", body.len());
    let _ = out.flush();
}

fn position(params: &Json) -> (usize, usize) {
    let at = &params["position"];
    (
        at["line"].as_u64().unwrap_or(0) as usize,
        at["character"].as_u64().unwrap_or(0) as usize,
    )
}

pub fn serve() {
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    let mut documents: HashMap<String, Document> = HashMap::new();

    while let Some(message) = read(&mut input) {
        let method = message["method"].as_str().unwrap_or("");
        let id = message.get("id").cloned();
        let params = &message["params"];
        let uri = params["textDocument"]["uri"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let mut publish = false;

        let result = match method {
            "initialize" => Some(json!({
                "capabilities": {
                    "textDocumentSync": 1,
                    "hoverProvider": true,
                    "documentFormattingProvider": true,
                    "completionProvider": { "triggerCharacters": ["."] },
                },
                "serverInfo": { "name": "khipu", "version": env!("CARGO_PKG_VERSION") },
            })),
            "shutdown" => Some(Json::Null),
            "exit" => return,
            "textDocument/didOpen" => {
                let text = params["textDocument"]["text"].as_str().unwrap_or("");
                documents.insert(uri.clone(), Document::new(text.to_string()));
                publish = true;
                None
            }
            "textDocument/didChange" => {
                let changes = params["contentChanges"].as_array();
                let text = changes
                    .and_then(|all| all.last())
                    .and_then(|c| c["text"].as_str());
                if let (Some(text), Some(document)) = (text, documents.get_mut(&uri)) {
                    document.update(text.to_string());
                    publish = true;
                }
                None
            }
            "textDocument/didClose" => {
                documents.remove(&uri);
                send(
                    &mut out,
                    json!({ "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                            "params": { "uri": uri, "diagnostics": [] } }),
                );
                None
            }
            "textDocument/hover" => {
                let (line, column) = position(params);
                let text = documents.get(&uri).and_then(|d| d.hover(line, column));
                Some(text.map_or(
                    Json::Null,
                    |value| json!({ "contents": { "kind": "markdown", "value": value } }),
                ))
            }
            "textDocument/formatting" => Some(
                documents
                    .get(&uri)
                    .and_then(|document| document.formatted())
                    .unwrap_or(json!([])),
            ),
            "textDocument/completion" => {
                let (line, column) = position(params);
                let items = documents.get(&uri).map(|d| d.completions(line, column));
                Some(json!(items.unwrap_or_default()))
            }
            _ => None,
        };

        if let Some(id) = id {
            let reply = match result {
                Some(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                None => json!({ "jsonrpc": "2.0", "id": id,
                                "error": { "code": -32601, "message": format!("{method} is not supported") } }),
            };
            send(&mut out, reply);
        }
        if publish && let Some(document) = documents.get(&uri) {
            send(
                &mut out,
                json!({ "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                        "params": { "uri": uri, "diagnostics": document.diagnostics() } }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "struct Point {\n    x: number,\n    y: number,\n\n    fn length(self) -> number {\n        return self.x + self.y\n    }\n}\n\nlet p = Point { x: 1, y: 2 }\nlet total: number = 3\n";

    #[test]
    fn reports_the_problem_with_its_range() {
        let document = Document::new("let a = 1\nlet b: string = a\n".to_string());
        let diagnostic = &document.diagnostics()[0];
        assert_eq!(diagnostic["message"], "'b' should be string, got number");
        assert_eq!(
            diagnostic["range"]["start"],
            json!({ "line": 1, "character": 4 })
        );
        assert_eq!(
            diagnostic["range"]["end"],
            json!({ "line": 1, "character": 5 })
        );
        assert_eq!(Document::new(SOURCE.to_string()).diagnostics(), json!([]));
    }

    #[test]
    fn hovers_names_members_and_types() {
        let document = Document::new(SOURCE.to_string());
        assert_eq!(
            document.hover(10, 5).unwrap(),
            "```khipu\ntotal: number\n```"
        );
        assert_eq!(document.hover(9, 4).unwrap(), "```khipu\np: Point\n```");
        assert_eq!(document.hover(5, 20).unwrap(), "```khipu\nx: number\n```");
        assert!(
            document
                .hover(9, 9)
                .unwrap()
                .contains("struct Point {\n    length: fn() -> number\n    x: number")
        );
        assert_eq!(document.hover(0, 0).as_deref(), None);
    }

    #[test]
    fn completes_members_even_while_the_line_is_broken() {
        let mut document = Document::new(SOURCE.to_string());
        document.update(format!("{SOURCE}p.\n"));
        assert!(document.error.is_some());
        let labels: Vec<String> = document
            .completions(11, 2)
            .iter()
            .map(|item| item["label"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(labels, ["length", "x", "y"]);

        let everything = document.completions(11, 0);
        let has = |label: &str| everything.iter().any(|item| item["label"] == label);
        assert!(has("total") && has("Point") && has("print") && has("while"));
    }

    #[test]
    fn formats_the_whole_document() {
        let edits = Document::new("let  a=1\n".to_string()).formatted().unwrap();
        assert_eq!(edits[0]["newText"], "let a = 1\n");
        assert!(
            Document::new("let s = \"open\n".to_string())
                .formatted()
                .is_none()
        );
    }

    #[test]
    fn speaks_json_rpc_over_content_length() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
        let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let message = read(&mut framed.as_bytes()).unwrap();
        assert_eq!(message["method"], "initialize");
        let mut out = Vec::new();
        send(&mut out, json!({ "id": 1 }));
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Content-Length: 8\r\n\r\n{\"id\":1}"
        );
    }
}
