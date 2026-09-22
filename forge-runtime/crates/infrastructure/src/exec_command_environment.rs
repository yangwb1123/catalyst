use std::{collections::HashSet, ffi::OsString, process::Command};

use forge_runtime_domain::execution::fabric::{
    ENVIRONMENT_DIGEST_ALGORITHM, EnvironmentDigest, MAX_ENVIRONMENT_DIGEST_ENTRIES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const ENVIRONMENT_API_VERSION: &str = "forgeos.command-capture.environment/v1";
const ENVIRONMENT_CANONICALIZATION: &str = "forgeos.canonical-json/v1";
const ENVIRONMENT_PROFILE_ID: &str = "local-runtime-safe-environment-v1";
const ENVIRONMENT_DIGEST_DOMAIN: &[u8] = b"forgeos.runtime.local-environment-profile.v1\0";
const MAX_ENVIRONMENT_NAME_BYTES: usize = 128;
const MAX_ENVIRONMENT_VALUE_BYTES: usize = 16 * 1024;
const MAX_ENVIRONMENT_MANIFEST_BYTES: usize = 64 * 1024;

const SAFE_ENVIRONMENT_NAMES: &[&str] = &[
    "PATH",
    "HOME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "GOPATH",
    "GOMODCACHE",
    "GOCACHE",
    "PNPM_HOME",
    "LANG",
    "LC_ALL",
];
const SENSITIVE_ENVIRONMENT_MARKERS: &[&str] =
    &["KEY", "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "AUTH"];

#[derive(Clone, Serialize)]
struct EnvironmentVariable {
    name: String,
    value: String,
}

#[derive(Serialize)]
struct EnvironmentManifest {
    api_version: &'static str,
    canonicalization: &'static str,
    profile_id: &'static str,
    variables: Vec<EnvironmentVariable>,
}

/// The digest covers only the bounded allowlisted environment manifest. The
/// executable, argv, and workspace cwd remain separate local command inputs.
#[derive(Clone)]
pub(crate) struct SafeEnvironment {
    entries: Vec<EnvironmentVariable>,
    digest: EnvironmentDigest,
}

impl SafeEnvironment {
    pub(crate) fn capture() -> Self {
        capture_from(std::env::vars_os())
    }

    pub(crate) fn apply(&self, command: &mut Command) {
        for entry in &self.entries {
            command.env(&entry.name, &entry.value);
        }
    }

    pub(crate) fn digest(&self) -> EnvironmentDigest {
        self.digest.clone()
    }
}

fn capture_from<I>(variables: I) -> SafeEnvironment
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut failure = None;
    for (name, value) in variables {
        let Some(name) = name.to_str() else {
            failure.get_or_insert("safe environment name is not valid UTF-8");
            continue;
        };
        let Some(canonical_name) = canonical_environment_name(name) else {
            continue;
        };
        if !seen.insert(canonical_name) {
            failure.get_or_insert("safe environment contains duplicate names");
            continue;
        }
        if canonical_name.len() > MAX_ENVIRONMENT_NAME_BYTES {
            failure.get_or_insert("safe environment name exceeds capture bound");
            continue;
        }
        let Some(value) = value.to_str() else {
            failure.get_or_insert("safe environment value is not valid UTF-8");
            continue;
        };
        if value.len() > MAX_ENVIRONMENT_VALUE_BYTES {
            failure.get_or_insert("safe environment value exceeds capture bound");
            continue;
        }
        if entries.len() >= usize::from(MAX_ENVIRONMENT_DIGEST_ENTRIES) {
            failure.get_or_insert("safe environment contains too many entries");
            continue;
        }
        entries.push(EnvironmentVariable {
            name: canonical_name.to_owned(),
            value: value.to_owned(),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    let digest = match failure {
        Some(reason) => not_captured(reason),
        None if !entries.iter().any(|entry| entry.name == "PATH") => {
            not_captured("safe environment does not contain PATH")
        }
        None => captured_digest(&entries),
    };
    SafeEnvironment { entries, digest }
}

fn captured_digest(entries: &[EnvironmentVariable]) -> EnvironmentDigest {
    let manifest = EnvironmentManifest {
        api_version: ENVIRONMENT_API_VERSION,
        canonicalization: ENVIRONMENT_CANONICALIZATION,
        profile_id: ENVIRONMENT_PROFILE_ID,
        variables: entries.to_vec(),
    };
    let encoded = match serde_json::to_vec(&manifest) {
        Ok(encoded) if encoded.len() <= MAX_ENVIRONMENT_MANIFEST_BYTES => encoded,
        Ok(_) => return not_captured("safe environment digest manifest exceeds bound"),
        Err(_) => return not_captured("safe environment digest serialization failed"),
    };
    let mut hasher = Sha256::new();
    hasher.update(ENVIRONMENT_DIGEST_DOMAIN);
    hasher.update(encoded);
    let mut sha256 = String::with_capacity(64);
    for byte in hasher.finalize() {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        sha256.push(char::from(HEX[usize::from(byte >> 4)]));
        sha256.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    EnvironmentDigest::Captured {
        algorithm: ENVIRONMENT_DIGEST_ALGORITHM.into(),
        sha256,
        entry_count: u16::try_from(entries.len()).unwrap_or(MAX_ENVIRONMENT_DIGEST_ENTRIES),
    }
}

fn not_captured(reason: &str) -> EnvironmentDigest {
    EnvironmentDigest::NotCaptured {
        reason: reason
            .chars()
            .take(forge_runtime_domain::execution::fabric::MAX_ENVIRONMENT_DIGEST_REASON_BYTES)
            .collect(),
    }
}

#[cfg(test)]
pub(crate) fn is_allowed_environment_name(name: &str) -> bool {
    canonical_environment_name(name).is_some()
}

pub(crate) fn is_sensitive_environment_name(name: &str) -> bool {
    let uppercase = name.to_ascii_uppercase();
    SENSITIVE_ENVIRONMENT_MARKERS
        .iter()
        .any(|marker| uppercase.contains(marker))
}

fn canonical_environment_name(name: &str) -> Option<&'static str> {
    if is_sensitive_environment_name(name) {
        return None;
    }
    SAFE_ENVIRONMENT_NAMES
        .iter()
        .copied()
        .find(|allowed| name.eq_ignore_ascii_case(allowed))
}

pub(crate) fn is_valid_environment_digest(value: &EnvironmentDigest) -> bool {
    value.is_valid()
}

#[cfg(test)]
#[path = "exec_command_environment_tests.rs"]
mod tests;
