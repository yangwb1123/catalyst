package localcommandobservationproducer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// decodeCanonicalProduction validates one exact production wire without
// authenticating its producer or claiming that a process actually ran.
func decodeCanonicalProduction(data []byte) (ProductionPackage, error) {
	if len(data) == 0 || len(data) > maxManifestBytes {
		return ProductionPackage{}, fmt.Errorf("production JSON violates byte limits")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var value ProductionPackage
	if err := decoder.Decode(&value); err != nil {
		return ProductionPackage{}, fmt.Errorf("decode production JSON: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return ProductionPackage{}, fmt.Errorf("production JSON has trailing value")
	}
	if err := validateProductionPackage(value); err != nil {
		return ProductionPackage{}, err
	}
	canonical, err := canonicalManifest(value)
	if err != nil {
		return ProductionPackage{}, err
	}
	if !bytes.Equal(data, canonical) {
		return ProductionPackage{}, fmt.Errorf("production JSON is not exact compact canonical JSON")
	}
	return value, nil
}
