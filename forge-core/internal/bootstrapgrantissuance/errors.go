package bootstrapgrantissuance

import (
	"errors"
	"fmt"

	"forgeos/forge-core/internal/grantstate"
)

type Code string

const (
	CodeClockRejected        Code = "CLOCK_REJECTED"
	CodeIdempotencyConflict  Code = "IDEMPOTENCY_CONFLICT"
	CodeInvalidConfig        Code = "INVALID_CONFIG"
	CodeIssuerKeyRejected    Code = "ISSUER_KEY_REJECTED"
	CodeLedgerRejected       Code = "LEDGER_REJECTED"
	CodePersistenceFailed    Code = "PERSISTENCE_FAILED"
	CodePersistenceUncertain Code = "PERSISTENCE_UNCERTAIN"
	CodePolicyRejected       Code = "POLICY_REJECTED"
	CodeRequestRejected      Code = "REQUEST_REJECTED"
	CodeSigningRejected      Code = "SIGNING_REJECTED"
	CodeStateBusy            Code = "STATE_BUSY"
	CodeStateRejected        Code = "STATE_REJECTED"
	CodeTrustRootRejected    Code = "TRUST_ROOT_REJECTED"
)

type Error struct {
	Code Code
	Err  error
}

func (e *Error) Error() string {
	if !knownCode(e.Code) {
		return "bootstrap grant issuance: INTERNAL_ERROR"
	}
	return fmt.Sprintf("bootstrap grant issuance: %s", e.Code)
}

func (e *Error) Unwrap() error { return e.Err }

func ErrorCode(err error) Code {
	var target *Error
	if errors.As(err, &target) {
		return target.Code
	}
	return ""
}

func coded(code Code, err error) error {
	return &Error{Code: code, Err: err}
}

func knownCode(code Code) bool {
	switch code {
	case CodeClockRejected, CodeIdempotencyConflict, CodeInvalidConfig,
		CodeIssuerKeyRejected, CodeLedgerRejected, CodePersistenceFailed,
		CodePersistenceUncertain, CodePolicyRejected,
		CodeRequestRejected, CodeSigningRejected, CodeStateBusy,
		CodeStateRejected, CodeTrustRootRejected:
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
