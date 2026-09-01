// Package domain owns pure Delivery Domain v1 values and validation rules.
package domain

import "errors"

var (
	ErrInvalidDomain     = errors.New("delivery domain value is invalid")
	ErrInvalidTransition = errors.New("delivery domain transition is invalid")
)

type relationError struct {
	relation error
	detail   string
	cause    error
}

func (value *relationError) Error() string {
	message := value.relation.Error() + ": " + value.detail
	if value.cause != nil {
		message += ": " + value.cause.Error()
	}
	return message
}

func (value *relationError) Unwrap() error {
	return value.relation
}
