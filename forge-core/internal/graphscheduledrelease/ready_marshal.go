package graphscheduledrelease

// MarshalReadyAuthorization emits strict canonical JSON without a trailing LF.
func MarshalReadyAuthorization(value ReadyAuthorization) ([]byte, error) {
	if validateReadyAuthorization(value) != nil {
		return nil, errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxReadyAuthorizationBytes {
		return nil, errInvalidControl
	}
	return encoded, nil
}
