// Package authenticatedadrapprovalauthority authenticates and durably records
// the ADR-0079 approval-prerequisite wire. It never performs an ADR lifecycle
// transition or repository write.
package authenticatedadrapprovalauthority

import (
	"errors"
	"fmt"
)

// Code is a stable, non-secret failure classification.
type Code string

const (
	codeAuthorizationNotCurrent Code = "AUTHORIZATION_NOT_CURRENT"
	codeCapacityExhausted       Code = "CAPACITY_EXHAUSTED"
	codeCASConflict             Code = "CAS_CONFLICT"
	codeFixtureAuthority        Code = "FIXTURE_AUTHORITY_REJECTED"
	codeIdempotencyConflict     Code = "IDEMPOTENCY_CONFLICT"
	codeInputRejected           Code = "INPUT_REJECTED"
	codeInvalidConfig           Code = "INVALID_CONFIG"
	codeLedgerRejected          Code = "LEDGER_REJECTED"
	codePersistenceFailed       Code = "PERSISTENCE_FAILED"
	codePersistenceUncertain    Code = "PERSISTENCE_UNCERTAIN"
	codeProposalAlreadyAllowed  Code = "PROPOSAL_ALREADY_AUTHORIZED"
	codeProposalExcluded        Code = "PROPOSAL_EXCLUDED"
	codeRevocationRejected      Code = "REVOCATION_REJECTED"
	codeSignerKeyRejected       Code = "STATE_SIGNER_KEY_REJECTED"
	codeSigningRejected         Code = "SIGNING_REJECTED"
	codeSignatureRejected       Code = "SIGNATURE_REJECTED"
	codeStateBusy               Code = "STATE_BUSY"
	codeStateRejected           Code = "STATE_REJECTED"
	codeTimeRejected            Code = "TIME_REJECTED"
	codeTrustRootRejected       Code = "TRUST_ROOT_REJECTED"
	codeUnsupported             Code = "UNSUPPORTED_HOST"
)

type authorityError struct {
	Code Code
	Err  error
}

func (e *authorityError) Error() string {
	if !knownCode(e.Code) {
		return "authenticated ADR approval authority: INTERNAL_ERROR"
	}
	return fmt.Sprintf("authenticated ADR approval authority: %s", e.Code)
}

func (e *authorityError) Unwrap() error { return e.Err }

// ErrorCode returns an empty Code for errors not created by this package.
func ErrorCode(err error) Code {
	var target *authorityError
	if errors.As(err, &target) {
		return target.Code
	}
	return ""
}

func coded(code Code, err error) error { return &authorityError{Code: code, Err: err} }

func knownCode(code Code) bool {
	switch code {
	case codeAuthorizationNotCurrent, codeCapacityExhausted, codeCASConflict,
		codeFixtureAuthority, codeIdempotencyConflict, codeInputRejected,
		codeInvalidConfig, codeLedgerRejected, codePersistenceFailed,
		codePersistenceUncertain, codeProposalAlreadyAllowed, codeProposalExcluded,
		codeRevocationRejected, codeSignerKeyRejected, codeSigningRejected,
		codeSignatureRejected, codeStateBusy, codeStateRejected, codeTimeRejected,
		codeTrustRootRejected, codeUnsupported:
		return true
	default:
		return false
	}
}
