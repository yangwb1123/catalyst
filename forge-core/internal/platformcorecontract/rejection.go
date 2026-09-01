package platformcorecontract

import (
	"errors"
	"fmt"
)

// RejectionCode is the stable machine-readable Platform Core rejection class.
type RejectionCode string

const (
	rejectionDocumentInvalid   RejectionCode = "pc_document_invalid"
	rejectionIdentifierInvalid RejectionCode = "pc_identifier_invalid"
	rejectionValueInvalid      RejectionCode = "pc_value_invalid"
	rejectionReferenceMismatch RejectionCode = "pc_reference_mismatch"
	rejectionStateInvalid      RejectionCode = "pc_state_invalid"
	rejectionTransitionInvalid RejectionCode = "pc_transition_invalid"
	rejectionRelationMismatch  RejectionCode = "pc_relation_mismatch"
)

// ContractError carries one stable code and a non-stable human diagnostic.
type ContractError struct {
	Code   RejectionCode
	Detail string
}

func (value *ContractError) Error() string {
	return fmt.Sprintf("%s: %s", value.Code, value.Detail)
}

// RejectionCodeOf extracts the stable code without parsing diagnostic text.
func RejectionCodeOf(err error) (RejectionCode, bool) {
	var contractError *ContractError
	if !errors.As(err, &contractError) {
		return "", false
	}
	return contractError.Code, true
}

func reject(code RejectionCode, detail string) error {
	return &ContractError{Code: code, Detail: detail}
}

func rejectf(code RejectionCode, format string, arguments ...any) error {
	return reject(code, fmt.Sprintf(format, arguments...))
}

func withRejection(err error, fallback RejectionCode) error {
	if err == nil {
		return nil
	}
	if _, ok := RejectionCodeOf(err); ok {
		return err
	}
	return reject(fallback, err.Error())
}
