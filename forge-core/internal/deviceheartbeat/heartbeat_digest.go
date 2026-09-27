package deviceheartbeat

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
)

// Digest returns the stable digest of a canonical heartbeat value. A signed
// transport can bind this digest through its challenge before the pure
// heartbeat transition performs CAS and capability validation. The function
// is deterministic and has no clock, storage, or authority side effect.
func (value Heartbeat) Digest() (string, error) {
	capabilities, err := NewCapabilitySnapshot(
		value.Capabilities.OperatingSystem,
		value.Capabilities.Architecture,
		value.Capabilities.CPUCores,
		value.Capabilities.AvailableCPUCores,
		value.Capabilities.MemoryBytes,
		value.Capabilities.AvailableMemoryBytes,
		value.Capabilities.StorageBytes,
		value.Capabilities.AvailableStorageBytes,
		value.Capabilities.GPUs,
		value.Capabilities.Runtimes,
	)
	if err != nil {
		return "", err
	}
	canonical := value
	canonical.Capabilities = capabilities
	encoded, err := json.Marshal(canonical)
	if err != nil {
		return "", err
	}
	digest := sha256.Sum256(encoded)
	return hex.EncodeToString(digest[:]), nil
}
