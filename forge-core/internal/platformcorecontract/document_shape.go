package platformcorecontract

import "bytes"

func validateExactTypedDocument(data []byte, value any, maximum int, label string) error {
	canonical, err := typedCanonical(value, maximum)
	if err != nil {
		return withRejection(err, rejectionDocumentInvalid)
	}
	if !bytes.Equal(data, canonical) {
		return rejectf(rejectionDocumentInvalid, "%s is missing required exact fields", label)
	}
	return nil
}
