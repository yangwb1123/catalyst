package bootstrapgrantissuance

import (
	"encoding/hex"
	"fmt"
	"path"
	"path/filepath"
	"strings"
)

func validateConfig(config Config) error {
	if err := requireAbsolute(config.RepositoryRoot, "repository root"); err != nil {
		return err
	}
	if err := requireAbsolute(config.AuthorityRoot, "authority root"); err != nil {
		return err
	}
	if err := requireRelative(config.StateDir, "state directory"); err != nil {
		return err
	}
	leaves := []string{config.TrustRootPath, config.PolicyPath,
		config.RequestPath, config.IssuerSeedPath}
	seen := make(map[string]bool, len(leaves))
	for _, leaf := range leaves {
		if err := requireRelative(leaf, "authority leaf"); err != nil {
			return err
		}
		if seen[leaf] || pathWithin(leaf, config.StateDir) {
			return fmt.Errorf("authority leaves must be distinct and outside state")
		}
		seen[leaf] = true
	}
	return validatePin(config.PinnedRootSHA256)
}

func requireAbsolute(value, label string) error {
	if value == "" || !filepath.IsAbs(value) || filepath.Clean(value) != value ||
		strings.IndexByte(value, 0) >= 0 {
		return fmt.Errorf("%s must be an absolute canonical path", label)
	}
	return nil
}

func requireRelative(value, label string) error {
	hasDrive := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if value == "" || value == "." || strings.HasPrefix(value, "/") ||
		strings.Contains(value, "\\") || hasDrive || path.Clean(value) != value {
		return fmt.Errorf("%s must be a closed slash-relative path", label)
	}
	for _, component := range strings.Split(value, "/") {
		if component == "" || component == "." || component == ".." ||
			strings.IndexByte(component, 0) >= 0 {
			return fmt.Errorf("%s must be a closed slash-relative path", label)
		}
	}
	return nil
}

func validatePin(value string) error {
	decoded, err := hex.DecodeString(value)
	if err != nil || len(value) != 64 || len(decoded) != 32 || strings.ToLower(value) != value {
		return fmt.Errorf("pinned root SHA-256 must be 64 lowercase hexadecimal characters")
	}
	return nil
}

func pathWithin(value, directory string) bool {
	return value == directory || strings.HasPrefix(value, directory+"/")
}

func asciiLetter(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'
}
