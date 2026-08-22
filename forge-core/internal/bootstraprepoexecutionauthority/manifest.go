package bootstraprepoexecutionauthority

import "fmt"

var manifestKeys = []string{"api_version", "canonicalization", "entries", "kind", "manifest_sha256"}
var manifestEntryKeys = []string{"content_bytes", "content_sha256", "kind", "path"}

// Manifest is a strict caller-and-policy-bound expected-content precondition.
type Manifest struct{ document map[string]any }

// DecodeManifest validates canonical shape, bounds, ordering, and self digest.
func DecodeManifest(data []byte) (*Manifest, error) {
	document, err := decodeCanonical(data, maxManifestBytes)
	if err != nil {
		return nil, err
	}
	if err = validateManifest(document); err != nil {
		return nil, err
	}
	return &Manifest{document}, nil
}

func (manifest *Manifest) canonicalDocument() map[string]any { return cloneDocument(manifest.document) }

// Entries returns a detached exact read plan in manifest order.
func (manifest *Manifest) Entries() []struct {
	Path          string
	ContentBytes  int64
	ContentSHA256 string
} {
	if manifest == nil {
		return nil
	}
	entries, _ := arrayValue(manifest.document, "entries")
	result := make([]struct {
		Path          string
		ContentBytes  int64
		ContentSHA256 string
	}, 0, len(entries))
	for _, value := range entries {
		entry := value.(map[string]any)
		result = append(result, struct {
			Path          string
			ContentBytes  int64
			ContentSHA256 string
		}{Path: entry["path"].(string), ContentBytes: entry["content_bytes"].(int64),
			ContentSHA256: entry["content_sha256"].(string)})
	}
	return result
}

func validateManifest(document map[string]any) error {
	if err := requireKeys(document, manifestKeys...); err != nil {
		return fmt.Errorf("RepoReadExpectedManifest: %w", err)
	}
	if err := validateEnvelope(document, manifestAPI, "RepoReadExpectedManifest"); err != nil {
		return err
	}
	entries, err := arrayValue(document, "entries")
	if err != nil || len(entries) < 1 || len(entries) > 16 {
		return fmt.Errorf("Manifest entries must contain 1..16 items")
	}
	if err = validateManifestEntries(entries); err != nil {
		return err
	}
	claimed, err := stringValue(document, "manifest_sha256")
	if err != nil || validateHash(claimed, "manifest_sha256") != nil {
		return fmt.Errorf("Manifest self digest is invalid")
	}
	computed, err := selfDigest(manifestDomain, document, "manifest_sha256", maxManifestBytes,
		"RepoReadExpectedManifest", false, "")
	if err != nil || computed != claimed {
		return fmt.Errorf("Manifest self digest does not match")
	}
	return nil
}

func validateManifestEntries(entries []any) error {
	prior, total := "", int64(0)
	for index, value := range entries {
		entry, ok := value.(map[string]any)
		if !ok || requireKeys(entry, manifestEntryKeys...) != nil || entry["kind"] != "regular" {
			return fmt.Errorf("Manifest entry %d shape or kind is invalid", index)
		}
		path, pathErr := stringValue(entry, "path")
		bytes, bytesErr := intValue(entry, "content_bytes")
		if pathErr != nil || validateRepoPath(path) != nil || (index > 0 && prior >= path) ||
			bytesErr != nil || bytes < 0 || bytes > maxContentBytes {
			return fmt.Errorf("Manifest entry %d path, order, or byte count is invalid", index)
		}
		if err := validateHashField(entry, "content_sha256", "Manifest content_sha256"); err != nil {
			return err
		}
		if total > maxContentBytes-bytes {
			return fmt.Errorf("Manifest aggregate content exceeds %d", maxContentBytes)
		}
		prior, total = path, total+bytes
	}
	return nil
}

func validateManifestGrant(manifest *Manifest, grant *issuedGrant) error {
	entries, _ := arrayValue(manifest.document, "entries")
	resources, _ := arrayValue(grant.document, "resources")
	if len(entries) != len(resources) {
		return fmt.Errorf("Manifest path set differs from exact Grant scope")
	}
	for index := range entries {
		entry := entries[index].(map[string]any)
		resource := resources[index].(map[string]any)
		if entry["path"] != resource["path"] {
			return fmt.Errorf("Manifest path set differs from exact Grant scope")
		}
	}
	var total int64
	for _, value := range entries {
		bytes, _ := intValue(value.(map[string]any), "content_bytes")
		total += bytes
	}
	budget := grant.document["budget"].(map[string]any)
	limit, _ := intValue(budget, "max_output_bytes")
	if total > limit {
		return fmt.Errorf("Manifest aggregate bytes exceed Grant output budget")
	}
	return nil
}

func validateManifestAction(manifest *Manifest, actionValue any) error {
	action, ok := actionValue.(map[string]any)
	if !ok {
		return fmt.Errorf("requested_action must be an object")
	}
	resources, _ := arrayValue(action, "resources")
	entries, _ := arrayValue(manifest.document, "entries")
	if len(entries) != len(resources) {
		return fmt.Errorf("Manifest path set differs from requested_action")
	}
	var total int64
	for index, value := range entries {
		entry := value.(map[string]any)
		resource := resources[index].(map[string]any)
		if entry["path"] != resource["path"] {
			return fmt.Errorf("Manifest path set differs from requested_action")
		}
		bytes, _ := intValue(entry, "content_bytes")
		total += bytes
	}
	usage := action["usage"].(map[string]any)
	limit, _ := intValue(usage, "output_bytes")
	if total != limit {
		return fmt.Errorf("Manifest aggregate bytes differ from requested_action output usage")
	}
	return nil
}

func validateEnvelope(document map[string]any, api, kind string) error {
	for key, expected := range map[string]string{"api_version": api,
		"canonicalization": canonicalization, "kind": kind} {
		if err := requireLiteral(document, key, expected); err != nil {
			return err
		}
	}
	return nil
}
