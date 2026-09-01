package receipt

func validateVerificationRequest(value *VerificationRequest) error {
	_, err := CanonicalVerificationRequestJSON(value)
	return err
}

// CanonicalVerificationRequestJSON returns exact compact canonical v1 bytes.
func CanonicalVerificationRequestJSON(value *VerificationRequest) ([]byte, error) {
	if err := validateVerificationRequestShape(value); err != nil {
		return nil, err
	}
	if err := validateVerificationRequestValues(value); err != nil {
		return nil, err
	}
	canonical, err := typedCanonical(value, maxReceiptBytes)
	if err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	if err := validateVerificationInputReferences(value.ScopeRef, &value.InputArtifactRef); err != nil {
		return nil, err
	}
	if err := validateVerificationRequestRelations(value); err != nil {
		return nil, err
	}
	return canonical, nil
}

// DecodeCanonicalVerificationRequest accepts one exact canonical v1 request.
func DecodeCanonicalVerificationRequest(data []byte) (*VerificationRequest, error) {
	var value VerificationRequest
	if err := decodeTypedCanonical(data, maxReceiptBytes, &value); err != nil {
		return nil, withRejection(err, rejectionDocumentInvalid)
	}
	if err := validateExactTypedDocument(data, &value, "VerificationRequest"); err != nil {
		return nil, err
	}
	if err := validateVerificationRequestFields(&value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	return &value, nil
}

// VerificationRequestSHA256 returns a domain-separated conformance digest.
func VerificationRequestSHA256(value *VerificationRequest) (string, error) {
	canonical, err := CanonicalVerificationRequestJSON(value)
	if err != nil {
		return "", err
	}
	return observationDigest(verificationRequestDigestDomain, canonical), nil
}

func validateVerificationReceipt(value *VerificationReceipt) error {
	_, err := CanonicalVerificationReceiptJSON(value)
	return err
}

// CanonicalVerificationReceiptJSON returns exact compact canonical v1 bytes.
func CanonicalVerificationReceiptJSON(value *VerificationReceipt) ([]byte, error) {
	if err := validateVerificationReceiptShape(value); err != nil {
		return nil, err
	}
	if err := validateVerificationReceiptValues(value); err != nil {
		return nil, err
	}
	canonical, err := typedCanonical(value, maxReceiptBytes)
	if err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	if err := validateVerificationInputReferences(value.ScopeRef, &value.InputArtifactRef); err != nil {
		return nil, err
	}
	if err := validateVerificationReceiptState(value); err != nil {
		return nil, err
	}
	if err := validateVerificationReceiptRelations(value); err != nil {
		return nil, err
	}
	return canonical, nil
}

// DecodeCanonicalVerificationReceipt accepts one exact canonical v1 receipt.
func DecodeCanonicalVerificationReceipt(data []byte) (*VerificationReceipt, error) {
	var value VerificationReceipt
	if err := decodeTypedCanonical(data, maxReceiptBytes, &value); err != nil {
		return nil, withRejection(err, rejectionDocumentInvalid)
	}
	if err := validateExactTypedDocument(data, &value, "VerificationReceipt"); err != nil {
		return nil, err
	}
	if err := validateVerificationReceiptFields(&value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	return &value, nil
}

// VerificationReceiptSHA256 returns a domain-separated conformance digest.
func VerificationReceiptSHA256(value *VerificationReceipt) (string, error) {
	canonical, err := CanonicalVerificationReceiptJSON(value)
	if err != nil {
		return "", err
	}
	return observationDigest(verificationReceiptDigestDomain, canonical), nil
}
