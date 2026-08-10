package gitworktreesource

import (
	"fmt"
	"sort"
	"strings"
)

func Validate(manifest SourceManifest, digest string) error {
	if err := validateManifest(manifest); err != nil {
		return err
	}
	if !validDigest(digest) {
		return fmt.Errorf("source manifest digest is invalid")
	}
	actual, err := Digest(manifest)
	if err != nil {
		return err
	}
	if actual != digest {
		return fmt.Errorf("source manifest digest mismatch")
	}
	return nil
}

func validateManifest(manifest SourceManifest) error {
	if manifest.APIVersion != APIVersion || manifest.Canonicalization != Canonicalization ||
		manifest.ProfileID != ProfileID || !validSourceRevision(manifest.SourceRevision) {
		return fmt.Errorf("production source manifest fixed fields drifted")
	}
	if manifest.Entries == nil || len(manifest.Entries) > maxSourceEntries {
		return fmt.Errorf("production source entries are invalid")
	}
	paths := make([]string, len(manifest.Entries))
	var total int64
	for index, entry := range manifest.Entries {
		if err := validateManifestEntry(entry); err != nil {
			return err
		}
		if entry.Bytes > maxSourceBytes-total {
			return fmt.Errorf("production source manifest exceeds byte limit")
		}
		total += entry.Bytes
		paths[index] = entry.Path
	}
	if !sort.StringsAreSorted(paths) {
		return fmt.Errorf("production source entries are not path sorted")
	}
	for index := 1; index < len(paths); index++ {
		if paths[index] == paths[index-1] {
			return fmt.Errorf("production source entries contain duplicate path %q", paths[index])
		}
	}
	return nil
}

func validateManifestEntry(entry SourceEntry) error {
	tracked := entry.Tracking == "tracked"
	if !tracked && entry.Tracking != "untracked" {
		return fmt.Errorf("production source path %q has invalid tracking", entry.Path)
	}
	if err := validateInventoryPath(entry.Path, tracked); err != nil {
		return err
	}
	if pathWithin(foldASCII(entry.Path), ".forge") {
		return fmt.Errorf("production source path %q is forbidden control state", entry.Path)
	}
	if tracked {
		if entry.IndexMode == nil || !validIndexMode(*entry.IndexMode) {
			return fmt.Errorf("tracked source path %q has invalid index mode", entry.Path)
		}
	} else if entry.IndexMode != nil {
		return fmt.Errorf("untracked source path %q has index mode", entry.Path)
	}
	switch entry.Kind {
	case "regular":
		return validateRegularEntry(entry)
	case "symlink":
		return validateSymlinkEntry(entry)
	case "deleted":
		return validateDeletedEntry(entry)
	default:
		return fmt.Errorf("production source path %q has invalid kind", entry.Path)
	}
}

func validIndexMode(value string) bool {
	return value == "100644" || value == "100755" || value == "120000"
}

func validateRegularEntry(entry SourceEntry) error {
	if entry.Bytes < 0 || entry.Bytes > maxIndividualFileBytes || entry.ContentSHA256 == nil ||
		!validDigest(*entry.ContentSHA256) || entry.Executable == nil || entry.SymlinkTarget != nil {
		return fmt.Errorf("regular source path %q has invalid facts", entry.Path)
	}
	if entry.Tracking == "tracked" && entry.IndexMode != nil && *entry.IndexMode == "120000" {
		return fmt.Errorf("regular source path %q conflicts with symlink index mode", entry.Path)
	}
	return nil
}

func validateSymlinkEntry(entry SourceEntry) error {
	if entry.ContentSHA256 == nil || entry.Executable == nil || *entry.Executable ||
		entry.SymlinkTarget == nil {
		return fmt.Errorf("symlink source path %q has invalid facts", entry.Path)
	}
	if entry.Tracking == "tracked" && (entry.IndexMode == nil || *entry.IndexMode != "120000") {
		return fmt.Errorf("symlink source path %q conflicts with regular index mode", entry.Path)
	}
	target := *entry.SymlinkTarget
	if err := validateText("source symlink target", target, false); err != nil {
		return err
	}
	if entry.Bytes != int64(len([]byte(target))) || *entry.ContentSHA256 != sha256Bytes([]byte(target)) {
		return fmt.Errorf("symlink source path %q has inconsistent bytes or digest", entry.Path)
	}
	return nil
}

func validateDeletedEntry(entry SourceEntry) error {
	if entry.Tracking != "tracked" || entry.Bytes != 0 || entry.ContentSHA256 != nil ||
		entry.Executable != nil || entry.SymlinkTarget != nil {
		return fmt.Errorf("deleted source path %q has invalid facts", entry.Path)
	}
	return nil
}

func validDigest(value string) bool {
	return len(value) == 64 && lowerHex(value)
}

func validSourceRevision(value string) bool {
	if strings.HasPrefix(value, "git-sha1:") {
		return len(value) == len("git-sha1:")+40 && lowerHex(strings.TrimPrefix(value, "git-sha1:"))
	}
	if strings.HasPrefix(value, "git-sha256:") {
		return len(value) == len("git-sha256:")+64 && lowerHex(strings.TrimPrefix(value, "git-sha256:"))
	}
	return false
}
