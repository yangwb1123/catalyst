use std::{fs::File, path::PathBuf};

use super::{CoreTerminalBridgeError, invalid};

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        fs::File,
        io::{Read, Seek, Write},
        os::fd::AsRawFd,
        path::PathBuf,
    };

    use rustix::fs::{
        MemfdFlags, Mode, SealFlags, fchmod, fcntl_add_seals, fcntl_get_seals, memfd_create,
    };

    use super::{CoreTerminalBridgeError, invalid};
    use crate::core_terminal_bridge::{MAX_CORE_BINARY_BYTES, hash_bounded};

    const REQUIRED_SEALS: SealFlags = SealFlags::SEAL
        .union(SealFlags::SHRINK)
        .union(SealFlags::GROW)
        .union(SealFlags::WRITE)
        .union(SealFlags::EXEC);

    pub(super) struct PlatformExecutable {
        file: File,
    }

    impl PlatformExecutable {
        pub(super) fn prepare(
            source: File,
            size: u64,
            expected_sha256: &str,
        ) -> Result<Self, CoreTerminalBridgeError> {
            if size == 0 || size > MAX_CORE_BINARY_BYTES {
                return Err(invalid("Core executable size is invalid"));
            }
            let descriptor = memfd_create(
                "forge-core-terminal",
                MemfdFlags::ALLOW_SEALING | MemfdFlags::EXEC,
            )
            .map_err(|_| invalid("Core executable cannot be sealed"))?;
            let mut file = File::from(descriptor);
            let copied = std::io::copy(&mut source.take(MAX_CORE_BINARY_BYTES + 1), &mut file)
                .map_err(|_| invalid("Core executable cannot be copied"))?;
            if copied != size {
                return Err(invalid("Core executable changed during copy"));
            }
            seal(&mut file)?;
            file.rewind()
                .map_err(|_| invalid("Core executable cannot be inspected"))?;
            if hash_bounded(&mut file, size)? != expected_sha256 {
                return Err(invalid("Core executable identity disagrees"));
            }
            Ok(Self { file })
        }

        pub(super) fn command_path(&self) -> PathBuf {
            PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
        }
    }

    fn seal(file: &mut File) -> Result<(), CoreTerminalBridgeError> {
        file.flush()
            .and_then(|()| file.sync_all())
            .map_err(|_| invalid("Core executable cannot be synchronized"))?;
        fchmod(&*file, Mode::from_raw_mode(0o500))
            .map_err(|_| invalid("Core executable cannot be marked executable"))?;
        fcntl_add_seals(&*file, REQUIRED_SEALS)
            .map_err(|_| invalid("Core executable cannot be sealed"))?;
        let actual =
            fcntl_get_seals(&*file).map_err(|_| invalid("Core executable seals unavailable"))?;
        if !actual.contains(REQUIRED_SEALS) {
            return Err(invalid("Core executable seals disagree"));
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use std::{fs::File, path::PathBuf};

    use super::{CoreTerminalBridgeError, invalid};

    pub(super) struct PlatformExecutable;

    impl PlatformExecutable {
        pub(super) fn prepare(
            _source: File,
            _size: u64,
            _expected_sha256: &str,
        ) -> Result<Self, CoreTerminalBridgeError> {
            Err(invalid("sealed Core execution requires Linux"))
        }

        pub(super) fn command_path(&self) -> PathBuf {
            PathBuf::new()
        }
    }
}

pub(super) struct PinnedCoreExecutable {
    inner: platform::PlatformExecutable,
}

impl PinnedCoreExecutable {
    pub(super) fn prepare(
        source: File,
        size: u64,
        expected_sha256: &str,
    ) -> Result<Self, CoreTerminalBridgeError> {
        Ok(Self {
            inner: platform::PlatformExecutable::prepare(source, size, expected_sha256)?,
        })
    }

    pub(super) fn command_path(&self) -> PathBuf {
        self.inner.command_path()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::{
        fs::{self, File},
        os::unix::fs::PermissionsExt,
        process::Command,
    };

    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::PinnedCoreExecutable;

    #[test]
    fn path_replacement_cannot_change_the_prepared_executable() {
        let directory = tempdir().expect("temporary executable directory");
        let source = directory.path().join("core");
        let replacement = directory.path().join("replacement");
        let trusted = b"#!/bin/sh\nprintf '%s' 'trusted'\n";
        fs::write(&source, trusted).expect("trusted executable");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700))
            .expect("trusted executable mode");
        let file = File::open(&source).expect("open trusted executable");
        let expected = format!("{:x}", Sha256::digest(trusted));
        let pinned = PinnedCoreExecutable::prepare(file, trusted.len() as u64, &expected)
            .expect("prepare sealed executable");

        fs::write(&replacement, b"#!/bin/sh\nprintf '%s' 'replaced'\n")
            .expect("replacement executable");
        fs::rename(&replacement, &source).expect("replace source path");
        let output = Command::new(pinned.command_path())
            .output()
            .expect("execute sealed bytes");

        assert!(output.status.success());
        assert_eq!(output.stdout, b"trusted");
    }
}
