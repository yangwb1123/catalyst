use std::{
    io::{self, Write},
    process::ExitCode,
};

const NAME: &str = "forge-runtime";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn run(json: bool) -> ExitCode {
    if let Err(error) = write_output(json, &mut io::stdout().lock()) {
        eprintln!("failed to write version output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn write_output(json: bool, output: &mut impl Write) -> io::Result<()> {
    if json {
        writeln!(
            output,
            "{}",
            serde_json::json!({"name": NAME, "version": VERSION})
        )
    } else {
        writeln!(output, "{NAME} {VERSION}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_text_uses_the_package_version_and_one_newline() {
        let mut output = Vec::new();
        write_output(false, &mut output).unwrap();
        assert_eq!(
            output,
            format!("forge-runtime {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
    }

    #[test]
    fn version_json_has_only_the_stable_name_and_version_fields() {
        let mut output = Vec::new();
        write_output(true, &mut output).unwrap();
        assert_eq!(output.last(), Some(&b'\n'));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            serde_json::json!({"name": "forge-runtime", "version": env!("CARGO_PKG_VERSION")})
        );
    }

    #[test]
    fn version_output_returns_writer_failure() {
        for json in [false, true] {
            let mut output = io::Cursor::new(&mut [][..]);
            assert_eq!(
                write_output(json, &mut output).unwrap_err().kind(),
                io::ErrorKind::WriteZero
            );
        }
    }
}
