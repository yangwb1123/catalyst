package graphsnapshot

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
)

// Decode accepts only the unique canonical envelope reconstructed from its
// embedded exact ADR-0053 bytes. Re-sealing tampered fields is insufficient.
func Decode(envelopeJSON []byte) (*Production, error) {
	return decodeWithProfile(envelopeJSON, legacyProfile)
}

// DecodeTestSource accepts only the dedicated ADR-0066 transport/profile and
// never negotiates or falls back to the legacy endpoint.
func DecodeTestSource(envelopeJSON []byte) (*Production, error) {
	return decodeWithProfile(envelopeJSON, testSourceProfile)
}

func decodeWithProfile(envelopeJSON []byte, profile projectionProfile) (*Production, error) {
	unsupported, err := unsupportedEnvelopeJSONProfile(envelopeJSON, profile)
	if err != nil {
		return nil, err
	}
	if unsupported {
		return nil, fmt.Errorf("unsupported_profile: unsupported graph snapshot version or profile")
	}
	if err := validateEnvelopeJSONShapeForProfile(envelopeJSON, profile); err != nil {
		return nil, fmt.Errorf("graph snapshot envelope shape: %w", err)
	}
	value, err := decodeEnvelopeValue(envelopeJSON)
	if err != nil {
		return nil, err
	}
	requestJSON, err := canonicalJSON(value.Request, maxRequestBytes)
	if err != nil {
		return nil, fmt.Errorf("request exceeds projection contract: %w", err)
	}
	if _, err := canonicalJSON(value.Snapshot, maxSnapshotBytes); err != nil {
		return nil, fmt.Errorf("snapshot exceeds projection contract: %w", err)
	}
	graph, err := decodeExactGraph(value.Request.GraphObservationBase64URL)
	if err != nil {
		return nil, err
	}
	production, err := buildWithProfile(graph, value.Request.GraphObservationSHA256,
		value.Request.RunID, value.Request.ProjectID, profile)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(production.RequestJSON(), requestJSON) ||
		!bytes.Equal(production.JSON(), envelopeJSON) {
		return nil, fmt.Errorf("envelope does not exactly reconstruct from embedded graph")
	}
	return production, nil
}

func unsupportedEnvelopeJSONProfile(raw []byte, profile projectionProfile) (bool, error) {
	if err := validateDiscriminatorJSONShape(raw, maxEnvelopeBytes); err != nil {
		return false, fmt.Errorf("graph snapshot discriminator shape: %w", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		return false, fmt.Errorf("decode graph snapshot discriminator: %w", err)
	}
	if _, err := decoder.Token(); err != io.EOF {
		return false, fmt.Errorf("graph snapshot discriminator has trailing data")
	}
	canonical, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil || !bytes.Equal(canonical, raw) {
		return false, fmt.Errorf("graph snapshot envelope is not exact canonical JSON")
	}
	return unsupportedDiscriminatorMap(value, profile), nil
}

func unsupportedDiscriminatorMap(value map[string]any, profile projectionProfile) bool {
	request, _ := value["request"].(map[string]any)
	snapshot, _ := value["snapshot"].(map[string]any)
	return mismatchedString(value, "api_version", profile.apiVersion) ||
		mismatchedString(request, "api_version", profile.requestVersion) ||
		mismatchedString(request, "projector_profile_id", profile.profileID) ||
		mismatchedString(snapshot, "api_version", snapshotVersion) ||
		mismatchedString(snapshot, "profile_id", profile.profileID)
}

func mismatchedString(value map[string]any, key, expected string) bool {
	actual, exists := value[key].(string)
	return exists && actual != expected
}

func decodeEnvelopeValue(raw []byte) (Envelope, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value Envelope
	if err := decoder.Decode(&value); err != nil {
		return value, fmt.Errorf("decode graph snapshot envelope: %w", err)
	}
	if _, err := decoder.Token(); err != io.EOF {
		return value, fmt.Errorf("graph snapshot envelope has trailing data")
	}
	canonical, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil || !bytes.Equal(canonical, raw) {
		return value, fmt.Errorf("graph snapshot envelope is not exact canonical JSON")
	}
	return value, nil
}

func decodeExactGraph(encoded string) ([]byte, error) {
	if encoded == "" || len(encoded) > maxBase64Bytes {
		return nil, fmt.Errorf("embedded graph Base64URL is empty or oversized")
	}
	raw, err := base64.RawURLEncoding.DecodeString(encoded)
	if err != nil || len(raw) == 0 || len(raw) > maxGraphBytes ||
		base64.RawURLEncoding.EncodeToString(raw) != encoded {
		return nil, fmt.Errorf("embedded graph is not exact unpadded Base64URL")
	}
	return raw, nil
}
