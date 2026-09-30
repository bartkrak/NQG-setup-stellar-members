use risc0_build::{DockerOptionsBuilder, GuestOptionsBuilder};
use std::{collections::HashMap, path::PathBuf};

const GUEST_BUILDER: &str = "r0.1.97.0@sha256:6f6c300e4af57ea3efe62b7f04a17629efe0f0caa6ad3699d6766bf19e0e62fb";

fn main() {
    // Host Clippy must not replace the guest's RISC Zero compiler.
    std::env::remove_var("RUSTC_WORKSPACE_WRAPPER");
    println!("cargo:rerun-if-changed=../prior-voting-history-core/src");
    println!("cargo:rerun-if-changed=../prior-voting-history-core/Cargo.toml");
    println!("cargo:rerun-if-env-changed=NQG_REPRODUCIBLE");
    if std::env::var("NQG_REPRODUCIBLE").as_deref() == Ok("1") {
        std::env::set_var("RISC0_DOCKER_CONTAINER_TAG", GUEST_BUILDER);
        let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .parent()
            .unwrap()
            .to_path_buf();
        let docker = DockerOptionsBuilder::default()
            .root_dir(root)
            .docker_container_tag(GUEST_BUILDER)
            .build()
            .unwrap();
        let guest = GuestOptionsBuilder::default()
            .use_docker(docker)
            .build()
            .unwrap();
        risc0_build::embed_methods_with_options(HashMap::from([(
            "prior_voting_history_guest",
            guest,
        )]));
    } else {
        risc0_build::embed_methods();
    }
}
