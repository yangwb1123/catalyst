use std::{
    io::{ErrorKind, Write},
    process::{Child, Output},
};

pub(super) fn finish(mut child: Child, input: &[u8]) -> Output {
    let written = child.stdin.take().expect("stdin").write_all(input);
    let output = child.wait_with_output().expect("wait for CLI");
    if let Err(error) = written {
        // A rejected command may close stdin before reading it. The caller
        // must still check the exact rejection; success requires a complete write.
        assert_eq!(error.kind(), ErrorKind::BrokenPipe, "write stdin: {error}");
        assert!(
            !output.status.success(),
            "CLI succeeded without accepting all stdin"
        );
    }
    output
}

#[cfg(unix)]
mod tests {
    use std::{
        io::Read,
        panic::{AssertUnwindSafe, catch_unwind},
        process::{Child, Command, Stdio},
    };

    use super::finish;

    #[test]
    fn early_rejection_keeps_status_and_diagnostic_after_broken_pipe() {
        let output = finish(closed_stdin_child("23"), b"unread input");
        assert_eq!(output.status.code(), Some(23));
        assert_eq!(output.stderr, b"rejected");
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn successful_exit_cannot_hide_incomplete_input() {
        let child = closed_stdin_child("0");
        assert!(catch_unwind(AssertUnwindSafe(|| finish(child, b"unread input"))).is_err());
    }

    fn closed_stdin_child(status: &str) -> Child {
        let mut child = Command::new("/bin/sh")
            .args([
                "-c",
                "exec 0<&-; printf closed; printf rejected >&2; exit \"$1\"",
                "stdin-fixture",
                status,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn closed-stdin fixture");
        let mut signal = [0; 6];
        child
            .stdout
            .as_mut()
            .expect("fixture stdout")
            .read_exact(&mut signal)
            .expect("fixture closes stdin before signalling");
        assert_eq!(&signal, b"closed");
        child
    }
}
