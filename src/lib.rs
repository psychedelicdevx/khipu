pub mod checker;
pub mod compiler;
pub mod error;
pub mod format;
pub mod gc;
pub mod http;
pub mod interpreter;
#[cfg(not(target_arch = "wasm32"))]
pub mod jit;
pub mod kernel;
pub mod lexer;
pub mod lsp;
pub mod native;
pub mod package;
pub mod parser;
#[cfg(not(target_arch = "wasm32"))]
pub mod poll;
pub mod stdlib;
pub mod task;
pub mod value;
pub mod vm;
pub mod wasmgen;
#[cfg(not(target_arch = "wasm32"))]
pub mod web;

#[cfg(target_arch = "wasm32")]
mod wasm;
