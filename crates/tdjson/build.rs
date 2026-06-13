//! Build script for libtdjson.
//!
//! Resolution order:
//!   1. `LIBTDJSON_PATH` env var  → direct path to `libtdjson.so` / `.dylib`
//!   2. `pkg-config`              → system-installed TDLib
//!   3. Fallback: tell Cargo to search standard paths

use std::env;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // 1. Explicit path via env var
    if let Ok(path) = env::var("LIBTDJSON_PATH") {
        let p = PathBuf::from(&path);
        let dir = p.parent().unwrap_or(&p);
        let stem = p.file_stem().unwrap().to_str().unwrap();
        // strip "lib" prefix on unix
        let name = stem.strip_prefix("lib").unwrap_or(stem);
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=dylib={}", name);
        println!("cargo:rerun-if-env-changed=LIBTDJSON_PATH");
        return;
    }

    // 2. pkg-config
    if pkg_config::Config::new()
        .atleast_version("1.8")
        .probe("tdjson")
        .is_ok()
    {
        return;
    }

    // 3. Fallback: just link tdjson and hope it's in the search path
    println!("cargo:rustc-link-lib=dylib=tdjson");
    if target_os == "macos" {
        println!("cargo:rustc-link-search=/usr/local/lib");
        println!("cargo:rustc-link-search=/opt/homebrew/lib");
    }
}
