package authenticatedadrlifecycleauthority

import (
	"errors"
	"fmt"
)

// Code is a stable, non-secret failure classification.
type Code string

const (
	codeAuthorizationRejected Code = "AUTHORIZATION_REJECTED"
	codeCapacityExhausted     Code = "CAPACITY_EXHAUSTED"
	codeCASConflict           Code = "CAS_CONFLICT"
	codeFixtureAuthority      Code = "FIXTURE_AUTHORITY_REJECTED"
	codeIdempotencyConflict   Code = "IDEMPOTENCY_CONFLICT"
	codeInputRejected         Code = "INPUT_REJECTED"
	codeInvalidConfig         Code = "INVALID_CONFIG"
	codePersistenceFailed     Code = "PERSISTENCE_FAILED"
	codePersistenceUncertain  Code = "PERSISTENCE_UNCERTAIN"
	codeProposalExcluded      Code = "PROPOSAL_EXCLUDED"
	codeSignerKeyRejected     Code = "STATE_SIGNER_KEY_REJECTED"
	codeSignatureRejected     Code = "SIGNATURE_REJECTED"
	codeStateBusy             Code = "STATE_BUSY"
	codeStateRejected         Code = "STATE_REJECTED"
	codeTimeRejected          Code = "TIME_REJECTED"
	codeTrustRootRejected     Code = "TRUST_ROOT_REJECTED"
	codeUnsupported           Code = "UNSUPPORTED_HOST"
)

type authorityError struct {
	Code Code
	Err  error
}

func (e *authorityError) Error() string {
	return fmt.Sprintf("authenticated ADR lifecycle authority: %s", e.Code)
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
