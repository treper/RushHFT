fn main() {
    // Tauri 2 looks for tauri.conf.json in the crate's CARGO_MANIFEST_DIR by
    // default. Our config lives in src-tauri/. Set TAURI_DIR to point there.
    println!(
        "cargo:rustc-env=TAURI_DIR={}/src-tauri",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    tauri_build::build();
}
