use std::{path::Path, process::Output};

use serde_json::Value;

pub(super) fn successful_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI JSON")
}

pub(super) fn path_text(path: &Path) -> &str {
    path.to_str().expect("test path is UTF-8")
}

pub(super) fn text(value: &Value) -> String {
    value.as_str().expect("JSON string").to_owned()
}
