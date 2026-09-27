package deviceplacement

// This file defines the explicit policy-complete source used by the fenced
// scheduler lease adapter.  The ordinary lifecycle inventory remains a
// display-only observation with unknown policy attributes; a deployment must
// provide this separate owner-private image before a lease can be claimed.

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"forgeos/forge-core/internal/statefs"
)

const (
	PlacementPolicyRegistrySchemaVersion  = "forge.device-placement-policy-registry/v1"
	PlacementPolicyRegistryEvaluationMode = "owner_bound_policy_attributes"
	maxPlacementPolicyRegistryFileBytes   = 1 << 20
)

// PlacementPolicyRegistry is an operator-published, owner-scoped policy image.
// It carries only the attributes that cannot be restored from the lifecycle
// heartbeat image.  The private file boundary and exact counter bindings keep
// a stale or foreign policy from being joined to a Runner observation.
type PlacementPolicyRegistry struct {
	SchemaVersion  string            `json:"schema_version"`
	EvaluationMode string            `json:"evaluation_mode"`
	Owner          Owner             `json:"owner"`
	Policies       []PlacementPolicy `json:"policies"`
}

// PlacementPolicy supplies the policy attributes required for scheduling.
// Every counter must equal the lifecycle observation being evaluated.
type PlacementPolicy struct {
	DeviceID           string   `json:"device_id"`
	InstanceID         string   `json:"instance_id"`
	Revision           uint64   `json:"revision"`
	Generation         uint64   `json:"generation"`
	HeartbeatSequence  uint64   `json:"heartbeat_sequence"`
	DataResidencyZones []string `json:"data_residency_zones"`
	TrustZone          string   `json:"trust_zone"`
	SandboxLevels      []string `json:"sandbox_levels"`
	ConcurrencyLimit   uint16   `json:"concurrency_limit"`
	ActiveConcurrency  uint16   `json:"active_concurrency"`
}

// PolicyRegistryError is a stable result for the private policy source.
type PolicyRegistryError string

const (
	ErrPlacementPolicyRegistryMissing PolicyRegistryError = "placement_policy_registry_missing"
	ErrPlacementPolicyRegistryInvalid PolicyRegistryError = "placement_policy_registry_invalid"
	ErrPlacementPolicyRegistryOwner   PolicyRegistryError = "placement_policy_registry_owner_mismatch"
	ErrPlacementPolicyRegistryBinding PolicyRegistryError = "placement_policy_registry_binding_mismatch"
)

func (e PolicyRegistryError) Error() string { return string(e) }

// ValidatePlacementPolicyRegistry checks the value without reading storage.
func ValidatePlacementPolicyRegistry(value PlacementPolicyRegistry) error {
	if value.SchemaVersion != PlacementPolicyRegistrySchemaVersion ||
		value.EvaluationMode != PlacementPolicyRegistryEvaluationMode ||
		!validOwner(value.Owner) || len(value.Policies) == 0 || len(value.Policies) > MaxDevices {
		return ErrPlacementPolicyRegistryInvalid
	}
	seenDevices := make(map[string]struct{}, len(value.Policies))
	seenInstances := make(map[string]struct{}, len(value.Policies))
	for index, policy := range value.Policies {
		if !validPlacementPolicy(policy) {
			return ErrPlacementPolicyRegistryInvalid
		}
		if index > 0 {
			prior := value.Policies[index-1]
			if prior.DeviceID > policy.DeviceID ||
				prior.DeviceID == policy.DeviceID && prior.InstanceID >= policy.InstanceID {
				return ErrPlacementPolicyRegistryInvalid
			}
		}
		if _, exists := seenDevices[policy.DeviceID]; exists {
			return ErrPlacementPolicyRegistryInvalid
		}
		if _, exists := seenInstances[policy.InstanceID]; exists {
			return ErrPlacementPolicyRegistryInvalid
		}
		seenDevices[policy.DeviceID] = struct{}{}
		seenInstances[policy.InstanceID] = struct{}{}
	}
	return nil
}

func validPlacementPolicy(value PlacementPolicy) bool {
	return validDeviceID(value.DeviceID) && validSessionIdentifier(value.InstanceID) &&
		value.Revision > 0 && value.Revision <= uint64(MaxSafeIntegerMS) &&
		value.Generation > 0 && value.Generation <= uint64(MaxSafeIntegerMS) &&
		value.HeartbeatSequence > 0 && value.HeartbeatSequence <= uint64(MaxSafeIntegerMS) &&
		len(value.DataResidencyZones) > 0 &&
		validUniqueTokens(value.DataResidencyZones, MaxArrayItems, validZone) &&
		validTrustZone(value.TrustZone) && len(value.SandboxLevels) > 0 &&
		validUniqueTokens(value.SandboxLevels, MaxArrayItems, validSandboxLevel) &&
		value.ConcurrencyLimit > 0 && value.ActiveConcurrency <= value.ConcurrencyLimit
}

// DecodePlacementPolicyRegistry strictly decodes one bounded policy image.
// Duplicate keys, unknown fields, null values, and trailing JSON are rejected
// before the value can be joined to lifecycle observations.
func DecodePlacementPolicyRegistry(data []byte) (PlacementPolicyRegistry, error) {
	if len(data) == 0 || len(data) > maxPlacementPolicyRegistryFileBytes || !utf8.Valid(data) ||
		rejectDuplicateFields(data) != nil || rejectNullValues(data) != nil {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryInvalid
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var value PlacementPolicyRegistry
	if err := decoder.Decode(&value); err != nil {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryInvalid
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryInvalid
	}
	if err := ValidatePlacementPolicyRegistry(value); err != nil {
		return PlacementPolicyRegistry{}, err
	}
	return value, nil
}

// PlacementPolicyRegistryFileReadAdapter reads one private, owner-bound policy
// image.  It never writes, refreshes, accepts heartbeats, or grants authority.
type PlacementPolicyRegistryFileReadAdapter struct {
	path  string
	owner Owner
}

func NewPlacementPolicyRegistryFileReadAdapter(path string, owner Owner) (PlacementPolicyRegistryFileReadAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) != path || !filepath.IsAbs(path) || !validOwner(owner) {
		return PlacementPolicyRegistryFileReadAdapter{}, ErrPlacementPolicyRegistryInvalid
	}
	return PlacementPolicyRegistryFileReadAdapter{path: path, owner: owner}, nil
}

func (adapter PlacementPolicyRegistryFileReadAdapter) Read() (PlacementPolicyRegistry, error) {
	file, present, err := statefs.InspectRegular(adapter.path)
	if err != nil {
		return PlacementPolicyRegistry{}, fmt.Errorf("%w: %v", ErrPlacementPolicyRegistryInvalid, err)
	}
	if !present {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryMissing
	}
	directory, directoryPresent, err := statefs.InspectDir(filepath.Dir(adapter.path))
	if err != nil || !directoryPresent || directory.Mode().Perm()&0o077 != 0 || file.Mode().Perm() != 0o600 {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryInvalid
	}
	data, present, err := statefs.ReadRegularUnmodified(adapter.path, maxPlacementPolicyRegistryFileBytes)
	if err != nil {
		return PlacementPolicyRegistry{}, fmt.Errorf("%w: %v", ErrPlacementPolicyRegistryInvalid, err)
	}
	if !present {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryMissing
	}
	value, err := DecodePlacementPolicyRegistry(data)
	if err != nil {
		return PlacementPolicyRegistry{}, err
	}
	if value.Owner != adapter.owner {
		return PlacementPolicyRegistry{}, ErrPlacementPolicyRegistryOwner
	}
	return value, nil
}
