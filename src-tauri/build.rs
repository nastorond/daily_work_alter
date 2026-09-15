use std::path::Path;

/// Filename of the report template inside `refs/`, relative to the repo root.
const TEMPLATE: &str = "../refs/0월 0일 주간업무 일지.hwpx";

fn main() {
    embed_template();
    tauri_build::build()
}

/// Bakes the 한글 report template into the binary so the exe stays a single
/// file to carry around.
///
/// The template is a company document and this repo is public, so `refs/` is
/// gitignored and the file only exists on the machine that owns it. A fresh
/// clone therefore builds fine — it just produces a binary with no template
/// baked in, which falls back to `%APPDATA%\DailyWorkAlter\template.hwpx` at
/// runtime (and that path wins even when one *is* baked in, so the template can
/// be swapped without a rebuild).
fn embed_template() {
    println!("cargo:rerun-if-changed={TEMPLATE}");

    let out = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out).join("bundled_template.rs");

    let src = Path::new(TEMPLATE);
    let body = if src.exists() {
        let abs = std::fs::canonicalize(src)
            .unwrap_or_else(|e| panic!("template at {TEMPLATE} could not be resolved: {e}"));
        format!(
            "pub const BUNDLED_TEMPLATE: Option<&[u8]> = Some(include_bytes!(r\"{}\"));",
            abs.display()
        )
    } else {
        println!("cargo:warning=refs/ 템플릿이 없어 한글 내보내기 템플릿을 내장하지 않습니다. \
                  실행 시 %APPDATA%\\DailyWorkAlter\\template.hwpx 를 사용합니다.");
        "pub const BUNDLED_TEMPLATE: Option<&[u8]> = None;".to_string()
    };

    std::fs::write(&dest, body).expect("failed to write bundled_template.rs");
}
