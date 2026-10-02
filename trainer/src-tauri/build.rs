use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    let package = output.join("rime-package");
    std::fs::create_dir_all(&package).expect("create immutable package build directory");
    let mut source = String::from("// Generated at build time, never during status/install.\n");
    source.push_str("pub const FILES: &[(&str, &str, &str)] = &[\n");
    let mut manifest = String::from("# filename\tsha256\tbytes\n");
    for artifact in xhup_generator::generate_rime_artifacts() {
        let path = package.join(artifact.filename());
        std::fs::create_dir_all(path.parent().expect("package parent")).unwrap();
        std::fs::write(&path, artifact.contents()).expect("write immutable package artifact");
        let hash = format!("{:x}", Sha256::digest(artifact.contents().as_bytes()));
        writeln!(
            &mut source,
            "({:?}, {:?}, include_str!({:?})),",
            artifact.filename(),
            hash,
            path
        )
        .unwrap();
        writeln!(
            &mut manifest,
            "{}\t{}\t{}",
            artifact.filename(),
            hash,
            artifact.contents().len()
        )
        .unwrap();
    }
    source.push_str("];\n");
    std::fs::write(output.join("bundled_package.rs"), source).unwrap();
    std::fs::write(output.join("bundled_package_manifest.tsv"), manifest).unwrap();
    tauri_build::build();
}
