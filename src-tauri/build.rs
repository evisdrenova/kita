use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Only compile on macOS
    #[cfg(target_os = "macos")]
    {
        // Paths to Swift files
        let swift_files = vec!["./src/swift/contacts.swift", "./src/swift/apps.swift"];

        // Check if Swift files exist
        for swift_file in &swift_files {
            if !Path::new(swift_file).exists() {
                panic!("Swift file not found: {}", swift_file);
            }
        }

        // Compile Swift to dynamic library
        let status = Command::new("swiftc")
            .args([
                "-emit-library",
                "-o",
                &format!("{}/libcontactsbridge.dylib", out_dir),
            ])
            .args(&swift_files)
            .args([
                "-framework",
                "Contacts",
                "-framework",
                "Foundation",
                "-framework",
                "AppKit",
                "-framework",
                "CoreGraphics",
            ])
            .status()
            .expect("Failed to compile Swift code");

        if !status.success() {
            panic!("Failed to compile Swift code");
        }

        // Tell cargo to link the library
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=dylib=contactsbridge");

        // Tell cargo to invalidate the built crate whenever the Swift files change
        for swift_file in &swift_files {
            println!("cargo:rerun-if-changed={}", swift_file);
        }
    }
    tauri_build::build()
}
