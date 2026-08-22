package projectsnapshot

import "fmt"

func validateManifest(value SourceManifest) error {
	if _, err := canonicalJSON(value, maxManifestBytes); err != nil {
		return fmt.Errorf("project source manifest canonical size is invalid: %w", err)
	}
	if value.APIVersion != manifestVersion || value.Canonicalization != canonicalization ||
		value.PathPolicyID != pathPolicyID || value.ProfileID != profileID ||
		!validRevision(value.SourceRevision) || value.Entries == nil || value.Excluded == nil ||
		len(value.Entries)+len(value.Excluded) > maxUniverseEntries ||
		len(value.Excluded) > maxExcludedEntries ||
		value.UniverseCount != int64(len(value.Entries)+len(value.Excluded)) ||
		value.IgnoredPathCount < 0 || value.IgnoredPathCount > maxIgnoredPaths {
		return fmt.Errorf("project source manifest fixed fields or bounds drifted")
	}
	entryDigests := make([]string, len(value.Entries))
	seen := make(map[string]struct{}, len(value.Entries)+len(value.Excluded))
	var total int64
	for index, entry := range value.Entries {
		if err := validateEntry(entry); err != nil {
			return err
		}
		if index > 0 && value.Entries[index-1].PathSHA256 >= entry.PathSHA256 {
			return fmt.Errorf("project source entries are not path-digest sorted")
		}
		if _, duplicate := seen[entry.PathSHA256]; duplicate {
			return fmt.Errorf("project source path digest is duplicated")
		}
		seen[entry.PathSHA256] = struct{}{}
		if entry.Bytes > maxTotalBytes-total {
			return fmt.Errorf("project source manifest exceeds total byte bound")
		}
		total += entry.Bytes
		entryDigests[index] = entry.EntrySHA256
	}
	if err := validateExclusions(value.Excluded, seen); err != nil {
		return err
	}
	return validateManifestSeals(value, entryDigests)
}

func validateEntry(value Entry) error {
	if err := validateInventoryPath(value.Path); err != nil {
		return err
	}
	if protectedPathReason(value.Path) != "" || value.PathSHA256 != pathDigest(value.Path) ||
		!validDigest(value.EntrySHA256) || value.Bytes < 0 || value.Bytes > maxFileBytes {
		return fmt.Errorf("project source entry path or bound is invalid")
	}
	if value.Tracking != "tracked" && value.Tracking != "untracked" ||
		value.Tracking == "tracked" && !validIndexMode(value.IndexMode) ||
		value.Tracking == "untracked" && value.IndexMode != nil {
		return fmt.Errorf("project source entry tracking or index mode is invalid")
	}
	if value.Kind == "regular" {
		if value.ContentSHA256 == nil || !validDigest(*value.ContentSHA256) ||
			value.Executable == nil || value.Tracking == "tracked" && *value.IndexMode == "120000" {
			return fmt.Errorf("project source regular entry facts are invalid")
		}
	} else if value.Kind != "tracked_absent" || value.Tracking != "tracked" || value.Bytes != 0 ||
		value.ContentSHA256 != nil || value.Executable != nil {
		return fmt.Errorf("project source absent entry facts are invalid")
	}
	expected := value
	expected.EntrySHA256 = ""
	digest, err := domainDigest(entryDomain, expected, maxManifestBytes)
	if err != nil || digest != value.EntrySHA256 {
		return fmt.Errorf("project source entry digest mismatch")
	}
	return nil
}

func validateExclusions(values []Exclusion, seen map[string]struct{}) error {
	for index, value := range values {
		if index > 0 && values[index-1].PathSHA256 >= value.PathSHA256 ||
			!validDigest(value.PathSHA256) || !validDigest(value.ExclusionSHA256) {
			return fmt.Errorf("project source exclusions are not path-digest sorted")
		}
		if _, duplicate := seen[value.PathSHA256]; duplicate {
			return fmt.Errorf("project source path digest is duplicated")
		}
		seen[value.PathSHA256] = struct{}{}
		if value.Tracking != "tracked" && value.Tracking != "untracked" ||
			value.Tracking == "tracked" && !validIndexMode(value.IndexMode) ||
			value.Tracking == "untracked" && value.IndexMode != nil {
			return fmt.Errorf("project source exclusion tracking is invalid")
		}
		if value.Reason != "symlink_leaf" && value.LeafFilesystemObserved ||
			value.Reason == "symlink_leaf" && !value.LeafFilesystemObserved ||
			value.Reason != "symlink_leaf" && value.Reason != "control_path" && value.Reason != "sensitive_path" {
			return fmt.Errorf("project source exclusion reason is invalid")
		}
		expected := value
		expected.ExclusionSHA256 = ""
		digest, err := domainDigest(exclusionDomain, expected, maxManifestBytes)
		if err != nil || digest != value.ExclusionSHA256 {
			return fmt.Errorf("project source exclusion digest mismatch")
		}
	}
	return nil
}

func validateManifestSeals(value SourceManifest, entryDigests []string) error {
	exclusionDigests := make([]string, len(value.Excluded))
	for index, item := range value.Excluded {
		exclusionDigests[index] = item.ExclusionSHA256
	}
	entrySet, entryErr := setDigest(entrySetDomain, entryDigests)
	exclusionSet, exclusionErr := setDigest(exclusionSetDomain, exclusionDigests)
	if entryErr != nil || exclusionErr != nil || entrySet != value.EntrySetSHA256 ||
		exclusionSet != value.ExclusionSetSHA256 {
		return fmt.Errorf("project source manifest set digest mismatch")
	}
	if err := validateGitObserver(value.GitObserver); err != nil {
		return err
	}
	expected := cloneManifest(value)
	expected.SourceManifestSHA256 = ""
	digest, err := domainDigest(manifestDomain, expected, maxManifestBytes)
	if err != nil || digest != value.SourceManifestSHA256 {
		return fmt.Errorf("project source manifest digest mismatch")
	}
	return nil
}

func validateGitObserver(value GitObserver) error {
	if value.ExecutableBytes < 1 || value.ExecutableBytes > maxGitExecutableBytes ||
		!validDigest(value.ExecutableSHA256) || value.IdentityAttestation != gitIdentityAttestation ||
		value.LocalConfigIsolation != gitLocalConfigIsolation ||
		value.NetworkContainment != gitNetworkContainment ||
		!validBoundedText(value.Version, maxShortTextBytes) || len(value.Version) <= len("git version ") ||
		value.Version[:len("git version ")] != "git version " {
		return fmt.Errorf("project source Git observer facts are invalid")
	}
	return nil
}

func validIndexMode(value *string) bool {
	return value != nil && (*value == "100644" || *value == "100755" || *value == "120000")
}
