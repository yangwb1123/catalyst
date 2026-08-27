use super::ScheduledGraphControllerServiceError;

pub(super) fn validate_identifier(value: &str) -> Result<(), ScheduledGraphControllerServiceError> {
    (!value.trim().is_empty() && value.len() <= 128 && !value.chars().any(unsupported_character))
        .then_some(())
        .ok_or(ScheduledGraphControllerServiceError::InvalidInput)
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

pub(super) fn validate_digest(value: &str) -> Result<(), ScheduledGraphControllerServiceError> {
    (value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(())
    .ok_or(ScheduledGraphControllerServiceError::InvalidInput)
}
