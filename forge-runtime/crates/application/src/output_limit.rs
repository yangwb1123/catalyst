use crate::runtime_domain::ToolOutput;

pub(crate) fn truncate_output(mut output: ToolOutput, max_bytes: usize) -> ToolOutput {
    if output.content.len() <= max_bytes {
        return output;
    }
    let mut boundary = max_bytes.min(output.content.len());
    while !output.content.is_char_boundary(boundary) {
        boundary = boundary.saturating_sub(1);
    }
    output.content.truncate(boundary);
    output.truncated = true;
    output
}

#[cfg(test)]
mod tests {
    use super::truncate_output;

    #[test]
    fn truncation_sets_the_flag_and_preserves_a_utf8_boundary() {
        let output = truncate_output(
            forge_runtime_domain::ToolOutput {
                content: "ééé".into(),
                truncated: false,
            },
            3,
        );
        assert_eq!(output.content, "é");
        assert!(output.truncated);
    }
}
