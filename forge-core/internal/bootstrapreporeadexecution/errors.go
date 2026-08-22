package bootstrapreporeadexecution

import (
	"errors"
	"fmt"

	"forgeos/forge-core/internal/grantstate"
)

type Code string

const (
	CodeClockRejected          Code = "CLOCK_REJECTED"
	CodeExecutionRootRejected  Code = "EXECUTION_ROOT_REJECTED"
	CodeGrantConsumed          Code = "GRANT_ALREADY_CONSUMED"
	CodeIdempotencyConflict    Code = "IDEMPOTENCY_CONFLICT"
	CodeInvalidConfig          Code = "INVALID_CONFIG"
	CodeInvocationRejected     Code = "INVOCATION_REJECTED"
	CodeIssuanceLedgerRejected Code = "ISSUANCE_LEDGER_REJECTED"
	CodeIssuanceRootRejected   Code = "ISSUANCE_ROOT_REJECTED"
	CodeManifestRejected       Code = "MANIFEST_REJECTED"
	CodeLedgerRejected         Code = "USAGE_LEDGER_REJECTED"
	CodePersistenceFailed      Code = "PERSISTENCE_FAILED"
	CodePersistenceUncertain   Code = "PERSISTENCE_UNCERTAIN"
	CodePolicyDenied           Code = "POLICY_DENIED"
	CodePolicyRejected         Code = "EXECUTION_POLICY_REJECTED"
	CodeRepositoryRejected     Code = "REPOSITORY_REJECTED"
	CodeSignerKeyRejected      Code = "EXECUTION_SIGNER_KEY_REJECTED"
	CodeStateBusy              Code = "STATE_BUSY"
	CodeStateRejected          Code = "STATE_REJECTED"
	CodeUnsupported            Code = "UNSUPPORTED_HOST"
)

type Error struct {
	Code Code
	Err  error
}

func (e *Error) Error() string {
	if !knownCode(e.Code) {
		return "bootstrap repository read execution: INTERNAL_ERROR"
	}
	return fmt.Sprintf("bootstrap repository read execution: %s", e.Code)
}

func (e *Error) Unwrap() error { return e.Err }

func ErrorCode(err error) Code {
	var target *Error
	if errors.As(err, &target) {
		return target.Code
	}
	return ""
}

func coded(code Code, err error) error { return &Error{Code: code, Err: err} }

func knownCode(code Code) bool {
	switch code {
	case CodeClockRejected, CodeExecutionRootRejected, CodeGrantConsumed,
		CodeIdempotencyConflict, CodeInvalidConfig, CodeInvocationRejected,
		CodeIssuanceLedgerRejected,
		CodeIssuanceRootRejected, CodeManifestRejected, CodePersistenceFailed,
		CodeLedgerRejected, CodePersistenceUncertain, CodePolicyDenied, CodePolicyRejected,
		CodeRepositoryRejected, CodeSignerKeyRejected, CodeStateBusy,
		CodeStateRejected, CodeUnsupported:
		return true
	default:
		return false
	}
}

func stateError(err error) error {
	switch grantstate.ErrorCode(err) {
	case grantstate.CodeBusy:
		return coded(CodeStateBusy, err)
	case grantstate.CodePersistenceUncertain:
		return coded(CodePersistenceUncertain, err)
	default:
		return coded(CodeStateRejected, err)
	}
}
