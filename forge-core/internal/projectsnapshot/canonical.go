package projectsnapshot

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
)

func canonicalJSON(value any, maximum int) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, fmt.Errorf("encode canonical project snapshot: %w", err)
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("canonical project snapshot encoder terminated unexpectedly")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) > maximum {
		return nil, fmt.Errorf("canonical project snapshot exceeds %d bytes", maximum)
	}
	return append([]byte(nil), encoded...), nil
}

func domainDigest(domain string, value any, maximum int) (string, error) {
	encoded, err := canonicalJSON(value, maximum)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write([]byte{0})
	_, _ = hasher.Write(encoded)
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

func pathDigest(value string) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(pathDomain))
	_, _ = hasher.Write([]byte{0})
	_, _ = hasher.Write([]byte(value))
	return hex.EncodeToString(hasher.Sum(nil))
}

type setPreimage struct {
	ItemCount int64    `json:"item_count"`
	Items     []string `json:"items"`
}

func setDigest(domain string, values []string) (string, error) {
	return domainDigest(domain, setPreimage{ItemCount: int64(len(values)), Items: values}, maxManifestBytes)
}

func cloneEnvelope(value Envelope) Envelope {
	value.Snapshot = cloneSnapshot(value.Snapshot)
	return value
}

func cloneSnapshot(value Snapshot) Snapshot {
	value.Coverage = cloneCoverage(value.Coverage)
	value.SourceManifest = cloneManifest(value.SourceManifest)
	return value
}

func cloneCoverage(value Coverage) Coverage {
	original := value.Surfaces
	value.Surfaces = make([]CoverageSurface, len(original))
	for index, surface := range original {
		surface.ReasonCodes = append([]string{}, surface.ReasonCodes...)
		value.Surfaces[index] = surface
	}
	return value
}

func cloneManifest(value SourceManifest) SourceManifest {
	originalEntries, originalExcluded := value.Entries, value.Excluded
	value.Entries = make([]Entry, len(originalEntries))
	for index, entry := range originalEntries {
		entry.ContentSHA256 = cloneString(entry.ContentSHA256)
		entry.Executable = cloneBool(entry.Executable)
		entry.IndexMode = cloneString(entry.IndexMode)
		value.Entries[index] = entry
	}
	value.Excluded = make([]Exclusion, len(originalExcluded))
	for index, exclusion := range originalExcluded {
		exclusion.IndexMode = cloneString(exclusion.IndexMode)
		value.Excluded[index] = exclusion
	}
	return value
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func cloneBool(value *bool) *bool {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func stringPointer(value string) *string { return &value }
func boolPointer(value bool) *bool       { return &value }

func sha256Bytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}
