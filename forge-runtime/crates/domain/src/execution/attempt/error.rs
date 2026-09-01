use std::fmt;

use crate::platform_core_contract::{PlatformCoreContractError, RejectionCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptRequestErrorCode {
    InvalidValue,
    ReferenceMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptRequestError {
    code: AttemptRequestErrorCode,
    message: String,
}

impl AttemptRequestError {
    pub(super) fn new(code: AttemptRequestErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> AttemptRequestErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<PlatformCoreContractError> for AttemptRequestError {
    fn from(value: PlatformCoreContractError) -> Self {
        let code = match value.code {
            RejectionCode::ReferenceMismatch | RejectionCode::RelationMismatch => {
                AttemptRequestErrorCode::ReferenceMismatch
            }
            RejectionCode::DocumentInvalid
            | RejectionCode::IdentifierInvalid
            | RejectionCode::ValueInvalid
            | RejectionCode::StateInvalid
            | RejectionCode::TransitionInvalid => AttemptRequestErrorCode::InvalidValue,
        };
        Self::new(code, value.message)
    }
}

impl fmt::Display for AttemptRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AttemptRequestError {}
