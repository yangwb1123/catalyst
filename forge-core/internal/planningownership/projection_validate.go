package planningownership

import (
	"bytes"
	"fmt"
)

// DecodeProjection strictly validates and fully reconstructs canonical projection bytes.
func DecodeProjection(data []byte) (Projection, error) {
	document, err := parseCanonicalObject(data, maxProjectionBytes)
	if err != nil {
		return Projection{}, err
	}
	requestObject, err := objectValue(document, "request")
	if err != nil {
		return Projection{}, err
	}
	requestBytes, err := canonicalJSON(requestObject)
	if err != nil {
		return Projection{}, err
	}
	request, err := DecodeRequest(requestBytes)
	if err != nil {
		return Projection{}, fmt.Errorf("embedded request rejected: %w", err)
	}
	expected, err := Project(request)
	if err != nil {
		return Projection{}, err
	}
	if !bytes.Equal(expected.encoded, data) {
		return Projection{}, fmt.Errorf("projection differs from complete pure reconstruction")
	}
	return Projection{document: document, encoded: cloneBytes(data)}, nil
}

// ValidateProjection revalidates an opaque projection and its complete reconstruction.
func ValidateProjection(projection Projection) error {
	if err := projection.valid(); err != nil {
		return err
	}
	decoded, err := DecodeProjection(projection.encoded)
	if err != nil || !bytes.Equal(decoded.encoded, projection.encoded) {
		return fmt.Errorf("projection integrity rejected")
	}
	return nil
}
