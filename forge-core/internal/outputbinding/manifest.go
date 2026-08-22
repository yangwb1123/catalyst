package outputbinding

import (
	"fmt"
	"path"
	"sort"
	"strings"
)

// SealManifest returns a detached, path-sorted, self-digested manifest.
func SealManifest(items []ManifestItem) (ArtifactManifest, error) {
	cloned := append([]ManifestItem(nil), items...)
	if cloned == nil {
		cloned = make([]ManifestItem, 0)
	}
	sort.Slice(cloned, func(left, right int) bool { return cloned[left].Path < cloned[right].Path })
	manifest := ArtifactManifest{APIVersion: manifestAPI,
		Canonicalization: canonicalization, Items: cloned}
	if err := validateManifestPayload(manifest); err != nil {
		return ArtifactManifest{}, err
	}
	digest, err := manifestDigest(manifest)
	if err != nil {
		return ArtifactManifest{}, err
	}
	manifest.ManifestSHA256 = digest
	return manifest, nil
}

// ManifestDigest returns the stable digest of a sorted copy of items.
func ManifestDigest(items []ManifestItem) (string, error) {
	manifest, err := SealManifest(items)
	if err != nil {
		return "", err
	}
	return manifest.ManifestSHA256, nil
}

// ValidateManifest verifies shape, canonical order, bounds, and self-digest.
func ValidateManifest(manifest ArtifactManifest) error {
	if err := validateManifestPayload(manifest); err != nil {
		return err
	}
	if err := requireDigest("manifest_sha256", manifest.ManifestSHA256); err != nil {
		return err
	}
	digest, err := manifestDigest(manifest)
	if err != nil {
		return err
	}
	if digest != manifest.ManifestSHA256 {
		return fmt.Errorf("output binding: manifest_sha256 does not match canonical manifest")
	}
	return nil
}

func validateManifestPayload(manifest ArtifactManifest) error {
	if manifest.APIVersion != manifestAPI || manifest.Canonicalization != canonicalization {
		return fmt.Errorf("output binding: artifact manifest fixed fields drifted")
	}
	if manifest.Items == nil || len(manifest.Items) > maxManifestItems {
		return fmt.Errorf("output binding: artifact items must be a non-null array of at most %d items", maxManifestItems)
	}
	var prior string
	var total int64
	for index, item := range manifest.Items {
		if err := validateManifestItem(item); err != nil {
			return fmt.Errorf("output binding: artifact item %d: %w", index, err)
		}
		if index > 0 && prior >= item.Path {
			return fmt.Errorf("output binding: artifact item paths must be sorted and unique")
		}
		if item.Bytes > maxArtifactBytes-total {
			return fmt.Errorf("output binding: artifact manifest exceeds %d total bytes", maxArtifactBytes)
		}
		total += item.Bytes
		prior = item.Path
	}
	return nil
}

func validateManifestItem(item ManifestItem) error {
	if item.Bytes < 1 || item.Bytes > maxArtifactBytes {
		return fmt.Errorf("bytes must be in 1..%d", maxArtifactBytes)
	}
	if err := validateArtifactPath(item.Path); err != nil {
		return err
	}
	if err := requireDigest("sha256", item.SHA256); err != nil {
		return err
	}
	return nil
}

func validateArtifactPath(value string) error {
	if err := validateWireText(value, false, maxReferenceBytes); err != nil {
		return fmt.Errorf("path: %w", err)
	}
	hasDrivePrefix := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if hasDrivePrefix || strings.Contains(value, "\\") || strings.HasPrefix(value, "/") ||
		strings.HasSuffix(value, "/") || strings.Contains(value, "//") || path.Clean(value) != value {
		return fmt.Errorf("path must be a normalized repository-relative slash path")
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("path contains an unsafe segment")
		}
	}
	return nil
}

func asciiLetter(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func manifestDigest(manifest ArtifactManifest) (string, error) {
	manifest.ManifestSHA256 = ""
	encoded, err := canonicalJSON(manifest, maxManifestBytes)
	if err != nil {
		return "", err
	}
	return domainDigest(manifestDomain, encoded), nil
}

// CanonicalManifestJSON returns exact compact canonical bytes without an LF.
func CanonicalManifestJSON(manifest ArtifactManifest) ([]byte, error) {
	if err := ValidateManifest(manifest); err != nil {
		return nil, err
	}
	return canonicalJSON(manifest, maxManifestBytes)
}

// DecodeCanonicalManifest accepts only the exact v1 canonical wire form.
func DecodeCanonicalManifest(data []byte) (ArtifactManifest, error) {
	var manifest ArtifactManifest
	err := decodeExact(data, maxManifestBytes, &manifest,
		func() error { return ValidateManifest(manifest) },
		func() ([]byte, error) { return canonicalJSON(manifest, maxManifestBytes) })
	return manifest, err
}
