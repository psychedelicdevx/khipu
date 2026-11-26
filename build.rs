use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let runtime = "playground/khipu.wasm";
    println!("cargo:rerun-if-changed={runtime}");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("runtime.wasm");
    let for_web = env::var("TARGET").is_ok_and(|target| target.starts_with("wasm32"));
    let bytes = if for_web {
        Vec::new()
    } else {
        fs::read(runtime).unwrap_or_default()
    };
    fs::write(out, bytes).unwrap();
}
