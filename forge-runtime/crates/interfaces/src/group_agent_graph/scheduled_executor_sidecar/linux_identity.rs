use std::{fs, fs::File, io::Read, os::unix::ffi::OsStrExt, path::PathBuf};

use super::{
    ScheduledExecutorLiveness, ScheduledExecutorSidecarDocument, ScheduledExecutorSidecarError,
};

const MACHINE_ID_PATH: &str = "/etc/machine-id";
const BOOT_ID_PATH: &str = "/proc/sys/kernel/random/boot_id";
const PID_NAMESPACE_PATH: &str = "/proc/self/ns/pid";
const TIME_NAMESPACE_PATH: &str = "/proc/self/ns/time";
const MAX_IDENTITY_BYTES: usize = 128;
const MAX_PROC_STAT_BYTES: usize = 16 * 1024;

pub(super) struct LinuxExecutionIdentity {
    pub(super) machine_id: String,
    pub(super) boot_id: String,
    pub(super) pid_namespace_id: String,
    pub(super) time_namespace_id: String,
    pub(super) proc_start_ticks: u64,
}

pub(super) fn capture_current() -> Result<LinuxExecutionIdentity, ScheduledExecutorSidecarError> {
    let machine_id = read_trimmed(MACHINE_ID_PATH, MAX_IDENTITY_BYTES)?;
    let boot_id = read_boot_id()?;
    let pid_namespace_id = read_namespace_id(PID_NAMESPACE_PATH, "pid")?;
    let time_namespace_id = read_namespace_id(TIME_NAMESPACE_PATH, "time")?;
    let pid = std::process::id();
    validate_procfs_pid_view(pid)?;
    let proc_start_ticks =
        read_proc_start_ticks(pid)?.ok_or(ScheduledExecutorSidecarError::InvalidDocument)?;
    Ok(LinuxExecutionIdentity {
        machine_id,
        boot_id,
        pid_namespace_id,
        time_namespace_id,
        proc_start_ticks,
    })
}

pub(super) fn classify(
    document: &ScheduledExecutorSidecarDocument,
) -> Result<ScheduledExecutorLiveness, ScheduledExecutorSidecarError> {
    let machine_id = read_trimmed(MACHINE_ID_PATH, MAX_IDENTITY_BYTES)?;
    if machine_id != document.linux_machine_id {
        return Err(ScheduledExecutorSidecarError::ForeignMachine);
    }
    let boot_id = read_boot_id()?;
    if boot_id != document.linux_boot_id {
        return Ok(ScheduledExecutorLiveness::Dead);
    }
    if read_namespace_id(PID_NAMESPACE_PATH, "pid")? != document.linux_pid_namespace_id {
        return Err(ScheduledExecutorSidecarError::ForeignPidNamespace);
    }
    if read_namespace_id(TIME_NAMESPACE_PATH, "time")? != document.linux_time_namespace_id {
        return Err(ScheduledExecutorSidecarError::ForeignTimeNamespace);
    }
    validate_procfs_pid_view(std::process::id())?;
    let observed = read_proc_start_ticks(document.pid)?;
    Ok(classify_observation(
        &document.linux_boot_id,
        &boot_id,
        document.proc_start_ticks,
        observed,
    ))
}

fn validate_procfs_pid_view(pid: u32) -> Result<(), ScheduledExecutorSidecarError> {
    let target =
        fs::read_link("/proc/self").map_err(|_| ScheduledExecutorSidecarError::UnsafeProcfsView)?;
    let observed = parse_proc_self_pid(target.as_os_str().as_bytes())?;
    (observed == pid)
        .then_some(())
        .ok_or(ScheduledExecutorSidecarError::UnsafeProcfsView)
}

fn parse_proc_self_pid(bytes: &[u8]) -> Result<u32, ScheduledExecutorSidecarError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| ScheduledExecutorSidecarError::UnsafeProcfsView)?;
    let pid = text
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(ScheduledExecutorSidecarError::UnsafeProcfsView)?;
    (pid.to_string() == text)
        .then_some(pid)
        .ok_or(ScheduledExecutorSidecarError::UnsafeProcfsView)
}

fn read_namespace_id(path: &str, namespace: &str) -> Result<String, ScheduledExecutorSidecarError> {
    let target = fs::read_link(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ScheduledExecutorSidecarError::NotFound
        } else {
            ScheduledExecutorSidecarError::Io
        }
    })?;
    let bytes = target.as_os_str().as_bytes();
    let value =
        std::str::from_utf8(bytes).map_err(|_| ScheduledExecutorSidecarError::InvalidDocument)?;
    valid_namespace_id(value, namespace)
        .then(|| value.to_owned())
        .ok_or(ScheduledExecutorSidecarError::InvalidDocument)
}

pub(super) fn valid_pid_namespace_id(value: &str) -> bool {
    valid_namespace_id(value, "pid")
}

pub(super) fn valid_time_namespace_id(value: &str) -> bool {
    valid_namespace_id(value, "time")
}

fn valid_namespace_id(value: &str, namespace: &str) -> bool {
    let prefix = format!("{namespace}:[");
    let Some(inode) = value
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    inode
        .parse::<u64>()
        .ok()
        .filter(|parsed| *parsed > 0 && parsed.to_string() == inode)
        .is_some()
}

pub(super) fn valid_machine_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(is_lower_hex)
}

pub(super) fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                is_lower_hex(byte)
            }
        })
}

fn read_boot_id() -> Result<String, ScheduledExecutorSidecarError> {
    canonical_boot_id(read_trimmed(BOOT_ID_PATH, MAX_IDENTITY_BYTES)?)
}

fn canonical_boot_id(value: String) -> Result<String, ScheduledExecutorSidecarError> {
    valid_boot_id(&value)
        .then_some(value)
        .ok_or(ScheduledExecutorSidecarError::InvalidDocument)
}

const fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (byte >= b'a' && byte <= b'f')
}

fn classify_observation(
    expected_boot_id: &str,
    current_boot_id: &str,
    expected_start_ticks: u64,
    observed_start_ticks: Option<u64>,
) -> ScheduledExecutorLiveness {
    if expected_boot_id != current_boot_id || observed_start_ticks.is_none() {
        return ScheduledExecutorLiveness::Dead;
    }
    if observed_start_ticks == Some(expected_start_ticks) {
        ScheduledExecutorLiveness::Live
    } else {
        ScheduledExecutorLiveness::PidReused
    }
}

fn read_proc_start_ticks(pid: u32) -> Result<Option<u64>, ScheduledExecutorSidecarError> {
    let path = PathBuf::from(format!("/proc/{pid}/stat"));
    let bytes = match read_bounded(&path, MAX_PROC_STAT_BYTES) {
        Ok(value) => value,
        Err(ScheduledExecutorSidecarError::NotFound) => return Ok(None),
        Err(error) => return Err(error),
    };
    let stat =
        std::str::from_utf8(&bytes).map_err(|_| ScheduledExecutorSidecarError::InvalidDocument)?;
    parse_start_ticks(stat).map(Some)
}

fn parse_start_ticks(stat: &str) -> Result<u64, ScheduledExecutorSidecarError> {
    let close = stat
        .rfind(')')
        .ok_or(ScheduledExecutorSidecarError::InvalidDocument)?;
    let fields = stat[close + 1..].split_whitespace().collect::<Vec<_>>();
    fields
        .get(19)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .ok_or(ScheduledExecutorSidecarError::InvalidDocument)
}

fn read_trimmed(path: &str, maximum: usize) -> Result<String, ScheduledExecutorSidecarError> {
    let bytes = read_bounded(PathBuf::from(path).as_path(), maximum)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| ScheduledExecutorSidecarError::InvalidDocument)?
        .trim();
    if value.is_empty() || value.len() > maximum {
        return Err(ScheduledExecutorSidecarError::InvalidDocument);
    }
    Ok(value.to_owned())
}

fn read_bounded(
    path: &std::path::Path,
    maximum: usize,
) -> Result<Vec<u8>, ScheduledExecutorSidecarError> {
    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ScheduledExecutorSidecarError::NotFound
        } else {
            ScheduledExecutorSidecarError::Io
        }
    })?;
    let mut bytes = Vec::with_capacity(maximum.min(256));
    file.take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ScheduledExecutorSidecarError::Io)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(ScheduledExecutorSidecarError::InvalidDocument);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_with_spaces_and_closing_parenthesis_in_name() {
        let prefix = "42 (worker ) name) R";
        let mut fields = (4_u64..=21)
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        fields.push("987654".into());
        fields.push("23".into());
        let stat = format!("{prefix} {}", fields.join(" "));
        assert_eq!(parse_start_ticks(&stat).expect("start ticks"), 987_654);
    }

    #[test]
    fn classifies_live_dead_and_pid_reuse_without_guessing() {
        assert_eq!(
            classify_observation("boot-a", "boot-a", 50, Some(50)),
            ScheduledExecutorLiveness::Live
        );
        assert_eq!(
            classify_observation("boot-a", "boot-a", 50, None),
            ScheduledExecutorLiveness::Dead
        );
        assert_eq!(
            classify_observation("boot-a", "boot-b", 50, Some(50)),
            ScheduledExecutorLiveness::Dead
        );
        assert_eq!(
            classify_observation("boot-a", "boot-a", 50, Some(51)),
            ScheduledExecutorLiveness::PidReused
        );
    }

    #[test]
    fn proc_self_target_is_exact_positive_decimal_pid() {
        assert_eq!(parse_proc_self_pid(b"123").expect("PID"), 123);
        for invalid in [b"".as_slice(), b"0", b"0123", b"-1", b"1/2", b"x"] {
            assert_eq!(
                parse_proc_self_pid(invalid).expect_err("invalid proc self target"),
                ScheduledExecutorSidecarError::UnsafeProcfsView
            );
        }
    }

    #[test]
    fn namespace_targets_are_exact_typed_positive_decimal_inodes() {
        assert!(valid_pid_namespace_id("pid:[123]"));
        assert!(valid_time_namespace_id("time:[123]"));
        for (namespace, invalid) in [
            ("pid", "pid:[0]"),
            ("pid", "pid:[0123]"),
            ("pid", "pid:[18446744073709551616]"),
            ("time", "time:[0123]"),
            ("time", "time_for_children:[123]"),
            ("time", "time:[x]"),
        ] {
            assert!(!valid_namespace_id(invalid, namespace));
        }
    }

    #[test]
    fn current_boot_id_must_be_canonical_before_reboot_classification() {
        let valid = "12345678-1234-1234-1234-123456789abc";
        assert_eq!(canonical_boot_id(valid.into()).expect("boot ID"), valid);
        for invalid in ["", "boot-b", "12345678-1234-1234-1234-123456789ABC"] {
            assert_eq!(
                canonical_boot_id(invalid.into()).expect_err("invalid boot ID"),
                ScheduledExecutorSidecarError::InvalidDocument
            );
        }
    }
}
