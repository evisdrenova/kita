fn main() {
    println!("cargo::rustc-check-cfg=cfg(coverage_nightly)");
    tauri_build::build()
}
