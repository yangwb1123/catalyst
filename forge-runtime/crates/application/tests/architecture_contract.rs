use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

const LOCAL_CRATES: [&str; 3] = [
    "forge-runtime-application",
    "forge-runtime-domain",
    "forge-runtime-infrastructure",
];

#[test]
fn production_dependencies_point_inward() {
    let metadata = cargo_metadata(&workspace_root());
    let cases = [
        ("forge-runtime-domain", set([])),
        ("forge-runtime-application", set(["forge-runtime-domain"])),
        (
            "forge-runtime-infrastructure",
            set(["forge-runtime-domain"]),
        ),
        (
            "forge-runtime-cli",
            set([
                "forge-runtime-application",
                "forge-runtime-domain",
                "forge-runtime-infrastructure",
            ]),
        ),
    ];

    for (package, expected) in cases {
        assert_eq!(
            local_dependencies(&metadata, package),
            expected,
            "unexpected production-layer dependency in {package}"
        );
    }
}

#[test]
fn core_layers_do_not_depend_on_the_filesystem_adapter() {
    let metadata = cargo_metadata(&workspace_root());
    for package in ["forge-runtime-domain", "forge-runtime-application"] {
        assert!(
            !dependency_names(&metadata, package).contains("cap-std"),
            "{package} must consume the domain workspace port, not cap-std"
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cargo_metadata(workspace: &Path) -> Value {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(workspace)
        .output()
        .expect("cargo metadata starts");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata is valid JSON")
}

fn local_dependencies(metadata: &Value, package: &str) -> BTreeSet<String> {
    dependency_names(metadata, package)
        .into_iter()
        .filter(|name| LOCAL_CRATES.contains(&name.as_str()))
        .collect()
}

fn dependency_names(metadata: &Value, package: &str) -> BTreeSet<String> {
    metadata["packages"]
        .as_array()
        .expect("metadata packages")
        .iter()
        .find(|candidate| candidate["name"] == package)
        .unwrap_or_else(|| panic!("metadata package '{package}'"))["dependencies"]
        .as_array()
        .expect("metadata dependencies")
        .iter()
        .filter(|dependency| dependency["kind"] != "dev")
        .filter_map(|dependency| dependency["name"].as_str())
        .map(str::to_owned)
        .collect()
}

fn set<const N: usize>(values: [&str; N]) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}
