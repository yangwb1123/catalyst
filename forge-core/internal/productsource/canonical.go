package productsource

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"

	"forgeos/forge-core/internal/gitworktreesource"
)

const maxManifestBytes = 16 << 20

func CanonicalJSON(manifest Manifest) ([]byte, error) {
	if err := validateManifest(manifest); err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(manifest); err != nil {
		return nil, fmt.Errorf("encode product source manifest: %w", err)
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("product source encoder did not terminate predictably")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) > maxManifestBytes {
		return nil, fmt.Errorf("product source manifest exceeds %d bytes", maxManifestBytes)
	}
	return append([]byte(nil), encoded...), nil
}

func Digest(manifest Manifest) (string, error) {
	encoded, err := CanonicalJSON(manifest)
	if err != nil {
		return "", err
	}
	digest := sha256.New()
	_, _ = digest.Write([]byte(DigestDomain))
	_, _ = digest.Write([]byte{0})
	_, _ = digest.Write(encoded)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func Validate(manifest Manifest, expected string) error {
	actual, err := Digest(manifest)
	if err != nil {
		return err
	}
	if actual != expected {
		return fmt.Errorf("product source digest mismatch")
	}
	return nil
}

func validateManifest(manifest Manifest) error {
	if manifest.APIVersion != APIVersion || manifest.Canonicalization != Canonicalization ||
		manifest.ProfileID != ProfileID || manifest.Entries == nil {
		return fmt.Errorf("product source manifest fixed fields drifted")
	}
	for _, entry := range manifest.Entries {
		if err := validateProductPath(entry.Path); err != nil {
			return err
		}
		if excludedProductPath(entry.Path) {
			return fmt.Errorf("product source contains excluded path %q", entry.Path)
		}
	}
	base := gitworktreesource.SourceManifest{
		APIVersion:       gitworktreesource.APIVersion,
		Canonicalization: gitworktreesource.Canonicalization,
		Entries:          manifest.Entries, ProfileID: gitworktreesource.ProfileID,
		SourceRevision: manifest.SourceRevision,
	}
	digest, err := gitworktreesource.Digest(base)
	if err != nil {
		return fmt.Errorf("validate product source entries: %w", err)
	}
	return gitworktreesource.Validate(base, digest)
}
