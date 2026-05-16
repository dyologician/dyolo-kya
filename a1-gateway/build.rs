/// build.rs — A1 Gateway build script
///
/// Assembles `studio/index.html` from the organized source files in
/// `studio/src/` **before** the Rust crate is compiled. The assembled file
/// is then embedded into the binary at compile-time via:
///
///   include_str!("../../../../studio/index.html")   (studio.rs)
///
/// This means every `cargo build` or `cargo run` automatically produces a
/// fresh Studio — no manual `./scripts/build-studio.sh` call required.
///
/// Source layout (all files picked up automatically — just add files):
///   studio/src/index.template.html        HTML skeleton
///   studio/src/css/*.css                  Assembled A-Z
///   studio/src/js/[0-9]*.js              Root JS, assembled A-Z (99-app last)
///   studio/src/js/components/*.js         Component JS, assembled A-Z
///
/// cargo:rerun-if-changed directives watch every source file individually so
/// Cargo only re-runs this script when something actually changed.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Use CARGO_MANIFEST_DIR (the directory containing this Cargo.toml, i.e.
    // a1-gateway/) for robust path resolution regardless of working directory.
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    );

    // a1-gateway/ is one level inside the workspace root
    let workspace_root = manifest_dir
        .parent()
        .expect("a1-gateway must be inside a workspace");

    let studio_src = workspace_root.join("studio").join("src");
    let out_path   = workspace_root.join("studio").join("index.html");

    // Always re-run if this build script itself changes
    println!("cargo:rerun-if-changed=build.rs");

    let template_path = studio_src.join("index.template.html");
    println!("cargo:rerun-if-changed={}", template_path.display());

    // ── Collect CSS files ────────────────────────────────────────────────────
    let css_dir = studio_src.join("css");
    let mut css_files = collect_and_watch(&css_dir, "css");
    css_files.sort();

    let mut css = String::new();
    for path in &css_files {
        css.push_str(
            &fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display())),
        );
    }

    // ── Collect JS files ─────────────────────────────────────────────────────
    let js_dir = studio_src.join("js");

    // Root JS files: numbered (01-xx through 07-xx), excluding 99-app.js
    let mut js_root: Vec<PathBuf> = collect_and_watch(&js_dir, "js")
        .into_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with(|c: char| c.is_ascii_digit()) && !name.starts_with("99-")
        })
        .collect();
    js_root.sort();

    // Component JS files come after root JS
    let comp_dir = js_dir.join("components");
    let mut js_comps = collect_and_watch(&comp_dir, "js");
    js_comps.sort();

    // 99-app.js is always last
    let js_app = js_dir.join("99-app.js");
    println!("cargo:rerun-if-changed={}", js_app.display());

    let mut js = String::new();
    for path in js_root.iter().chain(js_comps.iter()).chain(std::iter::once(&js_app)) {
        js.push_str(
            &fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display())),
        );
    }

    // ── Read template and substitute ─────────────────────────────────────────
    let template = fs::read_to_string(&template_path)
        .unwrap_or_else(|e| panic!("Cannot read template {}: {e}", template_path.display()));

    let html = template
        .replace("/* {{CSS}} */", &css)
        .replace("// {{JS}}", &js);

    // ── Write output ─────────────────────────────────────────────────────────
    fs::write(&out_path, &html)
        .unwrap_or_else(|e| panic!("Cannot write {}: {e}", out_path.display()));

    eprintln!(
        "  [build-studio] {} CSS + {} JS files → studio/index.html ({} bytes)",
        css_files.len(),
        js_root.len() + js_comps.len() + 1, // +1 for 99-app.js
        html.len(),
    );
}

/// Read all `*.{ext}` files in `dir`, emit a `cargo:rerun-if-changed` for
/// each one (so Cargo tracks individual file edits), and return the paths.
/// Also watches the directory itself so adding a new file triggers a rebuild.
fn collect_and_watch(dir: &Path, ext: &str) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return vec![];
    }

    // Watch the directory entry — triggers when files are added or removed
    println!("cargo:rerun-if-changed={}", dir.display());

    let files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Cannot read dir {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == ext)
                    .unwrap_or(false)
        })
        .collect();

    // Watch each file individually — triggers when file contents change
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    files
}
