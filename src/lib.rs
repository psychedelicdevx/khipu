pub mod checker;
pub mod compiler;
pub mod error;
pub mod format;
pub mod interpreter;
pub mod lexer;
pub mod lsp;
pub mod parser;
pub mod value;
pub mod vm;

#[cfg(target_arch = "wasm32")]
mod wasm;
