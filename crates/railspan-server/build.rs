fn main() {
    // Rebuild when embedded Vue assets change (rust-embed reads static/ at compile time).
    println!("cargo:rerun-if-changed=static/");
    println!("cargo:rerun-if-changed=static/index.html");
    println!("cargo:rerun-if-changed=static/assets/");
}
