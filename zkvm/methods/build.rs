use risc0_build::{embed_methods_with_options, DockerOptionsBuilder, GuestOptionsBuilder};
use std::{collections::HashMap, path::PathBuf};

// The locked guest dependencies require Rust >= 1.90, so use this official
// builder instead of risc0-build 3.0.6's Rust 1.88 default. Pin the digest so
// a moved tag cannot change the compiler or the resulting Image IDs.
const GUEST_BUILDER: &str =
    "r0.1.91.1@sha256:fafb377a44e1cfca415577c48d2f7012bda99ed36f2fae27f9a663b9fe6048f0";

fn main() {
    std::env::remove_var("RUSTC_WORKSPACE_WRAPPER");
    println!("cargo:rerun-if-changed=../core/prior-voting-history/src");
    println!("cargo:rerun-if-changed=../core/prior-voting-history/Cargo.toml");
    println!("cargo:rerun-if-changed=../core/assigned-reputation/src");
    println!("cargo:rerun-if-changed=../core/assigned-reputation/Cargo.toml");
    println!("cargo:rerun-if-env-changed=RISC0_DOCKER_CONTAINER_TAG");
    
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    let docker = DockerOptionsBuilder::default()
        .root_dir(manifest_dir.join(".."))
        .docker_container_tag(GUEST_BUILDER)
        .build()
        .expect("invalid guest Docker options");
    let opts = GuestOptionsBuilder::default()
        .use_docker(docker)
        .build()
        .expect("invalid guest build options");

    embed_methods_with_options(HashMap::from([
        ("prior_voting_history_guest", opts.clone()),
        ("assigned_reputation_guest", opts),
    ]));
}
