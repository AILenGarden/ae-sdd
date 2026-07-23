use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
    );
    let skill = manifest_dir.join("../../source/SKILL.md");
    println!("cargo:rerun-if-changed={}", skill.display());
    let contents = std::fs::read_to_string(&skill).expect("source/SKILL.md must be readable");
    let version = contents
        .lines()
        .find_map(|line| line.strip_prefix("version:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .expect("source/SKILL.md must declare version");
    println!("cargo:rustc-env=AE_SDD_PRODUCT_VERSION={version}");
}
