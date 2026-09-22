package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// DecodeSessionPlacementObservationRequest decodes the request accepted by the
// authenticated session device-observation preview. It applies the same
// bounded, duplicate-free, non-null, nested-shape contract as Decode for the
// standalone placement preview. Keeping this boundary strict prevents a
// missing nested field from becoming a valid Go zero value while the request
// is translated between clients.
func DecodeSessionPlacementObservationRequest(reader io.Reader) (SessionPlacementObservationRequest, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxRequestBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxRequestBytes || !utf8.Valid(data) {
		return SessionPlacementObservationRequest{}, errInvalidRequest
	}
	if rejectDuplicateFields(data) != nil || rejectNullValues(data) != nil || !requiredSessionPlacementObservationShape(data) {
		return SessionPlacementObservationRequest{}, errInvalidRequest
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var request SessionPlacementObservationRequest
	if decoder.Decode(&request) != nil {
		return SessionPlacementObservationRequest{}, errInvalidRequest
	}
	var trailing any
	if decoder.Decode(&trailing) != io.EOF {
		return SessionPlacementObservationRequest{}, errInvalidRequest
	}
	return request, nil
}

func requiredSessionPlacementObservationShape(data []byte) bool {
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || root == nil || !hasFields(root,
		"owner", "conversation_id", "run_id", "placement", "candidates") {
		return false
	}
	owner, okOwner := objectField(root, "owner")
	placement, okPlacement := objectField(root, "placement")
	candidates, okCandidates := arrayField(root, "candidates")
	if !okOwner || !hasFields(owner, "issuer", "subject", "tenant_id") ||
		!okPlacement || !requiredRequestObjectShape(placement) || !okCandidates || len(candidates) > MaxDevices {
		return false
	}
	for _, encoded := range candidates {
		var candidate map[string]json.RawMessage
		if json.Unmarshal(encoded, &candidate) != nil || candidate == nil || !hasFields(candidate, "instance_id", "device") {
			return false
		}
		device, okDevice := objectField(candidate, "device")
		if !okDevice || !requiredDeviceObjectShape(device) {
			return false
		}
	}
	return true
}
