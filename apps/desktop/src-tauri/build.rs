fn main() {
    // Windows icons are PE resources, independent of the frontend artwork.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=../dist");
    tauri_build::build()
}
