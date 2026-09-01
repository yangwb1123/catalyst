use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{OnceLock, mpsc},
    thread,
    time::{Duration, Instant},
};

const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_WORKSPACE_PACKAGES: usize = 32;
const MAX_WORKSPACE_TARGETS: usize = 1_024;
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);
const EXPECTED_DEPENDENCY_FILES: &[(&str, &str)] = &[
    (
        "Cargo.lock",
        "5577c86546c551085da867a2d7d44cfbfd812c9670853fd0e1a7c94076b6b5f7",
    ),
    (
        "Cargo.toml",
        "8ed616ecb4fa042631ab449999d2a09d1936e9a815c96334ad06885e49992ea6",
    ),
    (
        "crates/application/Cargo.toml",
        "239e2c6c45f291343b263f636dfc3d9eee452f25edb05de5f4ae26e311163678",
    ),
    (
        "crates/domain/Cargo.toml",
        "a8bfc8ef4ecf35ba930dd5501329d842a9dee7a64901907b8e455fe83b10cae9",
    ),
    (
        "crates/infrastructure/Cargo.toml",
        "2e79441e4a502d4aeda8a1574db268ae744445e04f6ae529b1a4ed4661c49f69",
    ),
    (
        "crates/interfaces/Cargo.toml",
        "9123dddae67fa6369d3187cf3f6bfede26389bf9a8d3d0c9bce838bfe9bf908b",
    ),
];

static CARGO_METADATA: OnceLock<CargoMetadata> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub(super) struct CargoMetadata {
    pub(super) packages: Vec<CargoPackage>,
    pub(super) workspace_members: Vec<String>,
    pub(super) workspace_root: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(super) struct CargoPackage {
    dependencies: Vec<CargoDependency>,
    id: String,
    manifest_path: PathBuf,
    name: String,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    kind: Option<String>,
    name: String,
    rename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    crate_types: Vec<String>,
    kind: Vec<String>,
    src_path: PathBuf,
}

pub(super) fn cargo_metadata() -> &'static CargoMetadata {
    CARGO_METADATA.get_or_init(load_cargo_metadata)
}

pub(super) fn verify_dependency_manifests(metadata: &CargoMetadata) {
    let expected = EXPECTED_DEPENDENCY_FILES
        .iter()
        .map(|(path, _)| *path)
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::from(["Cargo.lock", "Cargo.toml"]);
    for package in workspace_packages(metadata) {
        actual.insert(relative_utf8(
            &metadata.workspace_root,
            &package.manifest_path,
        ));
    }
    assert_eq!(actual, expected, "dependency manifest inventory drift");
    for (relative, digest) in EXPECTED_DEPENDENCY_FILES {
        verify_file_digest(&metadata.workspace_root.join(relative), digest);
    }
    verify_no_repository_cargo_config(&metadata.workspace_root);
}

pub(super) fn validate_control_file(path: &Path, workspace: &Path) {
    let name = path.file_name().and_then(|value| value.to_str());
    if name == Some("Cargo.toml") {
        let relative = relative_utf8(workspace, path);
        assert!(
            EXPECTED_DEPENDENCY_FILES
                .iter()
                .any(|(expected, _)| *expected == relative),
            "unreviewed nested Cargo manifest: {}",
            path.display()
        );
    } else if name == Some("Cargo.lock") {
        assert_eq!(path, workspace.join("Cargo.lock"));
    }
}

pub(super) fn validate_workspace_inventory(metadata: &CargoMetadata, attempt: &Path) {
    assert!(metadata.packages.len() <= MAX_WORKSPACE_PACKAGES);
    assert!(metadata.workspace_members.len() <= MAX_WORKSPACE_PACKAGES);
    let packages = workspace_packages(metadata);
    assert_eq!(packages.len(), metadata.workspace_members.len());
    let target_count = packages
        .iter()
        .map(|package| package.targets.len())
        .sum::<usize>();
    assert!(target_count <= MAX_WORKSPACE_TARGETS);
    for package in packages {
        validate_package_targets(package, &metadata.workspace_root, attempt);
    }
}

pub(super) fn domain_dependency_names(metadata: &CargoMetadata) -> Vec<String> {
    metadata
        .packages
        .iter()
        .find(|package| package.name == "forge-runtime-domain")
        .expect("domain package in Cargo metadata")
        .dependencies
        .iter()
        .filter(|dependency| dependency.kind.as_deref() != Some("dev"))
        .map(|dependency| {
            dependency
                .rename
                .as_ref()
                .unwrap_or(&dependency.name)
                .replace('-', "_")
        })
        .collect()
}

fn workspace_packages(metadata: &CargoMetadata) -> Vec<&CargoPackage> {
    let members = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(members.len(), metadata.workspace_members.len());
    metadata
        .packages
        .iter()
        .filter(|package| members.contains(package.id.as_str()))
        .collect()
}

fn validate_package_targets(package: &CargoPackage, workspace: &Path, attempt: &Path) {
    let root = package.manifest_path.parent().expect("manifest parent");
    assert!(root.starts_with(workspace), "workspace member escapes root");
    assert_regular(&package.manifest_path, "package manifest");
    let mut production_targets = 0_usize;
    for target in &package.targets {
        assert!(!target.kind.iter().any(|kind| kind == "custom-build"));
        assert!(!target.crate_types.iter().any(|kind| kind == "proc-macro"));
        if target
            .kind
            .iter()
            .any(|kind| matches!(kind.as_str(), "test" | "bench"))
        {
            continue;
        }
        production_targets += 1;
        validate_production_target(target, root, workspace, attempt);
    }
    assert!(
        production_targets > 0,
        "{} has no production target",
        package.name
    );
}

fn validate_production_target(target: &CargoTarget, root: &Path, workspace: &Path, attempt: &Path) {
    assert!(
        target.src_path.starts_with(root),
        "target escapes package root"
    );
    assert!(
        target.src_path.starts_with(workspace),
        "target escapes workspace"
    );
    assert!(
        !target.src_path.starts_with(attempt),
        "target reuses Attempt source"
    );
    assert!(
        !target.src_path.components().any(|component| {
            matches!(component.as_os_str().to_str(), Some("tests" | "benches"))
        }),
        "production target reuses test or bench source"
    );
    assert_regular(&target.src_path, "target source");
}

fn verify_file_digest(path: &Path, expected: &str) {
    assert_regular(path, "dependency manifest");
    let mut bytes = Vec::new();
    File::open(path)
        .expect("open dependency manifest")
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .expect("read dependency manifest");
    assert!(u64::try_from(bytes.len()).expect("manifest length") <= MAX_MANIFEST_BYTES);
    let actual = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        actual,
        expected,
        "dependency manifest drift: {}",
        path.display()
    );
}

fn relative_utf8<'a>(root: &Path, path: &'a Path) -> &'a str {
    path.strip_prefix(root)
        .expect("manifest below workspace")
        .to_str()
        .expect("UTF-8 manifest path")
}

fn assert_regular(path: &Path, label: &str) {
    let metadata = std::fs::symlink_metadata(path).unwrap_or_else(|_| panic!("missing {label}"));
    assert!(metadata.file_type().is_file(), "{label} must be regular");
}

fn verify_no_repository_cargo_config(workspace: &Path) {
    for root in [Some(workspace), workspace.parent()].into_iter().flatten() {
        for name in ["config", "config.toml"] {
            let path = root.join(".cargo").join(name);
            match path.symlink_metadata() {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => panic!("repository Cargo config is outside the dependency proof"),
                Err(error) => panic!("inspect repository Cargo config: {error}"),
            }
        }
    }
}

fn load_cargo_metadata() -> CargoMetadata {
    let declared = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let mut child = spawn_metadata(&declared);
    let stdout = child.stdout.take().expect("cargo metadata stdout");
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(read_metadata_stdout(stdout));
    });
    let deadline = Instant::now() + METADATA_TIMEOUT;
    let status = wait_bounded(&mut child, deadline);
    let remaining = deadline.saturating_duration_since(Instant::now());
    let bytes = receiver
        .recv_timeout(remaining)
        .expect("cargo metadata stdout exceeded wall-clock limit");
    assert!(status.success(), "cargo metadata failed");
    let metadata: CargoMetadata = serde_json::from_slice(&bytes).expect("decode cargo metadata");
    assert_eq!(metadata.workspace_root, declared);
    metadata
}

fn spawn_metadata(root: &Path) -> Child {
    Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version=1",
            "--no-deps",
            "--locked",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn bounded cargo metadata")
}

fn read_metadata_stdout(stdout: impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    stdout
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .expect("read cargo metadata");
    assert!(u64::try_from(bytes.len()).expect("metadata length") <= MAX_METADATA_BYTES);
    bytes
}

fn wait_bounded(child: &mut Child, deadline: Instant) -> ExitStatus {
    loop {
        if let Some(status) = child.try_wait().expect("poll cargo metadata") {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("cargo metadata exceeded wall-clock limit");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
