package authenticatedadrapprovalauthority

import (
	"encoding/hex"
	"fmt"
	"path"
	"path/filepath"
	"sort"
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
	leaves := []string{config.TrustRootPath, config.StateSignerSeedPath}
	for _, leaf := range leaves {
		if err := requireRelative(leaf, "authority leaf"); err != nil {
			return err
		}
		if pathWithin(leaf, config.StateDir) {
			return fmt.Errorf("authority leaves must be outside approval state")
		}
	}
	if leaves[0] == leaves[1] {
		return fmt.Errorf("trust root and state signer leaves must be distinct")
	}
	return validateExclusions(config.ExtraExcludedProposalBindingSHA256s)
}

func validateExternalTrust(trust ExternalTrust) error {
	if err := validateHash(trust.PinnedTrustRootSHA256, "pinned trust root"); err != nil {
		return err
	}
	if trust.PinnedTrustEpoch < 1 {
		return fmt.Errorf("pinned trust epoch must be positive")
	}
	if trust.ObservedAtUnixMS < 0 {
		return fmt.Errorf("trusted observed time must be nonnegative")
	}
	if trust.RevocationHighWaterSequence < 1 {
		return fmt.Errorf("trusted revocation high-water sequence must be positive")
	}
	return validateHash(trust.RevocationHighWaterSHA256, "revocation high-water")
}

func validateExclusions(values []string) error {
	copyValues := append([]string(nil), values...)
	for _, value := range copyValues {
		if err := validateHash(value, "excluded proposal binding"); err != nil {
			return err
		}
	}
	sort.Strings(copyValues)
	for index := 1; index < len(copyValues); index++ {
		if copyValues[index-1] == copyValues[index] {
			return fmt.Errorf("excluded proposal bindings must be unique")
		}
	}
	return nil
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

func validateHash(value, label string) error {
	decoded, err := hex.DecodeString(value)
	if err != nil || len(value) != 64 || len(decoded) != 32 || strings.ToLower(value) != value {
		return fmt.Errorf("%s SHA-256 must be 64 lowercase hexadecimal characters", label)
	}
	return nil
}

func pathWithin(value, directory string) bool {
	return value == directory || strings.HasPrefix(value, directory+"/")
}

func asciiLetter(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'
}
