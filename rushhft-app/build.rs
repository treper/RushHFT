fn main() {
    // tauri.conf.json lives at the crate root (CARGO_MANIFEST_DIR), which is
    // Tauri 2's default lookup location. Don't set TAURI_DIR — that would
    // redirect lookups to a non-existent src-tauri/ folder and silently break
    // frontendDist resolution.
    tauri_build::build();
}
