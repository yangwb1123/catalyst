package devicefabricgate

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path/filepath"

	"forgeos/forge-core/internal/statefs"
)

// ActivationManifestSchemaVersion identifies the on-disk request envelope.
// The envelope is deliberately separate from Result: a manifest is an
// operator-supplied request and never constitutes evidence by itself.
const ActivationManifestSchemaVersion = "forge.device-fabric-activation-manifest/v1"

const MaxActivationManifestBytes int64 = 64 * 1024

// Manifest is the strict, owner-private representation of a future activation
// request. Every decision/evidence field is explicit so a missing field cannot
// silently acquire a zero-value meaning during a review.
type Manifest struct {
	SchemaVersion string   `json:"schema_version"`
	Mode          Mode     `json:"mode"`
	ADR0039       Decision `json:"adr_0039"`
	ADR0113       Decision `json:"adr_0113"`
	ADR0114       Decision `json:"adr_0114"`
	P4            Decision `json:"p4"`
	Evidence      Evidence `json:"evidence"`
}

// Request converts a validated manifest to the pure gate input.
func (manifest Manifest) Request() Request {
	return Request{
		Mode: manifest.Mode, ADR0039: manifest.ADR0039,
		ADR0113: manifest.ADR0113, ADR0114: manifest.ADR0114,
		P4: manifest.P4, Evidence: manifest.Evidence,
	}
}

// ParseManifest strictly decodes one manifest. It rejects duplicate keys at
// every nesting level, unknown or missing fields, trailing JSON, and an
// envelope schema from another contract version. It performs no I/O.
func ParseManifest(data []byte) (Manifest, error) {
	if len(data) == 0 || int64(len(data)) > MaxActivationManifestBytes {
		return Manifest{}, fmt.Errorf("activation manifest must be between 1 and %d bytes", MaxActivationManifestBytes)
	}
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return Manifest{}, fmt.Errorf("activation manifest duplicate or malformed JSON: %w", err)
	}
	var manifest Manifest
	if err := decodeExactObject(data, &manifest, []string{
		"schema_version", "mode", "adr_0039", "adr_0113", "adr_0114", "p4", "evidence",
	}); err != nil {
		return Manifest{}, fmt.Errorf("activation manifest: %w", err)
	}
	if manifest.SchemaVersion != ActivationManifestSchemaVersion {
		return Manifest{}, fmt.Errorf("activation manifest schema_version must be %q", ActivationManifestSchemaVersion)
	}
	if _, ok := map[Mode]struct{}{
		ModeOff: {}, ModeInventory: {}, ModeObserve: {}, ModeExecute: {}, ModeMigrate: {}, ModeFederate: {},
	}[manifest.Mode]; !ok {
		return Manifest{}, fmt.Errorf("activation manifest mode %q is invalid", manifest.Mode)
	}
	if err := validateDecisionObject(data, "adr_0039"); err != nil {
		return Manifest{}, err
	}
	if err := validateDecisionObject(data, "adr_0113"); err != nil {
		return Manifest{}, err
	}
	if err := validateDecisionObject(data, "adr_0114"); err != nil {
		return Manifest{}, err
	}
	if err := validateDecisionObject(data, "p4"); err != nil {
		return Manifest{}, err
	}
	if err := validateEvidenceObject(data); err != nil {
		return Manifest{}, err
	}
	return manifest, nil
}

// LoadManifestFile reads one owner-private, regular, non-symlink manifest.
// The read is identity-checked and side-effect free; the function never
// changes permissions or creates a parent directory.
func LoadManifestFile(path string) (Manifest, error) {
	if path == "" || filepath.Clean(path) == "." {
		return Manifest{}, fmt.Errorf("activation manifest path is empty")
	}
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil {
		return Manifest{}, fmt.Errorf("activation manifest parent: %w", err)
	}
	if !present {
		return Manifest{}, fmt.Errorf("activation manifest parent is missing")
	}
	if directory.Mode().Perm()&0o077 != 0 {
		return Manifest{}, fmt.Errorf("activation manifest parent must be private")
	}
	file, present, err := statefs.InspectRegular(path)
	if err != nil {
		return Manifest{}, fmt.Errorf("activation manifest file: %w", err)
	}
	if !present {
		return Manifest{}, fmt.Errorf("activation manifest file is missing")
	}
	if file.Mode().Perm()&0o077 != 0 {
		return Manifest{}, fmt.Errorf("activation manifest file must not be group/world readable")
	}
	data, present, err := statefs.ReadRegularUnmodified(path, MaxActivationManifestBytes)
	if err != nil {
		return Manifest{}, fmt.Errorf("read activation manifest: %w", err)
	}
	if !present {
		return Manifest{}, fmt.Errorf("activation manifest file is missing")
	}
	return ParseManifest(data)
}

func validateDecisionObject(data []byte, field string) error {
	object, err := objectField(data, field)
	if err != nil {
		return fmt.Errorf("activation manifest %s: %w", field, err)
	}
	var decision Decision
	if err := decodeExactObject(object, &decision, []string{
		"status", "acceptance_id", "accepted_at_unix_ms", "planning_only",
	}); err != nil {
		return fmt.Errorf("activation manifest %s: %w", field, err)
	}
	return nil
}

func validateEvidenceObject(data []byte) error {
	object, err := objectField(data, "evidence")
	if err != nil {
		return fmt.Errorf("activation manifest evidence: %w", err)
	}
	var evidence Evidence
	if err := decodeExactObject(object, &evidence, []string{
		"coordinator_owner_isolation", "device_identity_proof", "owner_approval_and_revocation",
		"heartbeat_cas_and_freshness", "inventory_owner_scope", "disabled_default_and_route_close",
		"security_review", "runner_isolation", "lease_fencing", "cancellation_and_uncertain_work",
		"vault_artifact_authorization", "audit_outbox",
	}); err != nil {
		return fmt.Errorf("activation manifest evidence: %w", err)
	}
	return nil
}

func objectField(data []byte, field string) ([]byte, error) {
	var fields map[string]json.RawMessage
	if err := decodeExactObject(data, &fields, []string{
		"schema_version", "mode", "adr_0039", "adr_0113", "adr_0114", "p4", "evidence",
	}); err != nil {
		return nil, err
	}
	value, ok := fields[field]
	if !ok {
		return nil, fmt.Errorf("missing field %q", field)
	}
	return value, nil
}

func decodeExactObject(data []byte, target any, required []string) error {
	var fields map[string]json.RawMessage
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	if err := decoder.Decode(&fields); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return fmt.Errorf("trailing JSON value")
		}
		return err
	}
	if fields == nil {
		return fmt.Errorf("object required")
	}
	requiredSet := make(map[string]struct{}, len(required))
	for _, key := range required {
		requiredSet[key] = struct{}{}
		if _, ok := fields[key]; !ok {
			return fmt.Errorf("missing field %q", key)
		}
	}
	for key := range fields {
		if _, ok := requiredSet[key]; !ok {
			return fmt.Errorf("unknown field %q", key)
		}
	}
	strict := json.NewDecoder(bytes.NewReader(data))
	strict.DisallowUnknownFields()
	strict.UseNumber()
	if err := strict.Decode(target); err != nil {
		return err
	}
	if err := strict.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return fmt.Errorf("trailing JSON value")
		}
		return err
	}
	return nil
}

func rejectDuplicateJSONKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := walkJSONValue(decoder); err != nil {
		return err
	}
	var trailing json.RawMessage
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("trailing JSON value")
		}
		return err
	}
	return nil
}

func walkJSONValue(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delim, composite := token.(json.Delim)
	if !composite {
		return nil
	}
	switch delim {
	case '{':
		seen := map[string]struct{}{}
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := keyToken.(string)
			if !ok {
				return fmt.Errorf("object key is not a string")
			}
			if _, exists := seen[key]; exists {
				return fmt.Errorf("duplicate JSON object key %q", key)
			}
			seen[key] = struct{}{}
			if err := walkJSONValue(decoder); err != nil {
				return err
			}
		}
		return expectClose(decoder, '}')
	case '[':
		for decoder.More() {
			if err := walkJSONValue(decoder); err != nil {
				return err
			}
		}
		return expectClose(decoder, ']')
	default:
		return fmt.Errorf("unexpected JSON delimiter %q", delim)
	}
}

func expectClose(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	if token != expected {
		return fmt.Errorf("unexpected JSON close delimiter %q", token)
	}
	return nil
}
