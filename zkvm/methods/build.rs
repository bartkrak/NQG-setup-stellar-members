fn main() {
    // Host Clippy must not replace the guest's RISC Zero compiler.
    std::env::remove_var("RUSTC_WORKSPACE_WRAPPER");
    println!("cargo:rerun-if-changed=../core/prior-voting-history/src");
    println!("cargo:rerun-if-changed=../core/prior-voting-history/Cargo.toml");
    println!("cargo:rerun-if-changed=../core/assigned-reputation/src");
    println!("cargo:rerun-if-changed=../core/assigned-reputation/Cargo.toml");
    risc0_build::embed_methods();
}
