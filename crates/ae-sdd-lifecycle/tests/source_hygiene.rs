use std::{fs, path::Path};

#[test]
fn production_sources_use_typed_projection_and_have_no_panic_paths() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let entries = fs::read_dir(&source_dir).expect("lifecycle source directory is readable");
    let forbidden = [
        "serde_json",
        ".expect(",
        ".unwrap(",
        "panic!",
        "unreachable!",
        "process::abort",
        "process::exit",
    ];

    for entry in entries {
        let path = entry.expect("source entry is readable").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("Rust source is readable");
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "{} contains forbidden production construct {needle}",
                path.display()
            );
        }
    }
}
