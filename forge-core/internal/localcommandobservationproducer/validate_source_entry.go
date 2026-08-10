package localcommandobservationproducer

import "fmt"

func validateSourceManifestEntry(entry SourceEntry) error {
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
		return validateRegularSourceEntry(entry)
	case "symlink":
		return validateSymlinkSourceEntry(entry)
	case "deleted":
		return validateDeletedSourceEntry(entry)
	default:
		return fmt.Errorf("production source path %q has invalid kind", entry.Path)
	}
}

func validIndexMode(value string) bool {
	return value == "100644" || value == "100755" || value == "120000"
}

func validateRegularSourceEntry(entry SourceEntry) error {
	if entry.Bytes < 0 || entry.Bytes > maxIndividualFileBytes || entry.ContentSHA256 == nil ||
		!validDigest(*entry.ContentSHA256) || entry.Executable == nil || entry.SymlinkTarget != nil {
		return fmt.Errorf("regular source path %q has invalid facts", entry.Path)
	}
	if entry.Tracking == "tracked" && entry.IndexMode != nil && *entry.IndexMode == "120000" {
		return fmt.Errorf("regular source path %q conflicts with symlink index mode", entry.Path)
	}
	return nil
}

func validateSymlinkSourceEntry(entry SourceEntry) error {
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

func validateDeletedSourceEntry(entry SourceEntry) error {
	if entry.Tracking != "tracked" || entry.Bytes != 0 || entry.ContentSHA256 != nil ||
		entry.Executable != nil || entry.SymlinkTarget != nil {
		return fmt.Errorf("deleted source path %q has invalid facts", entry.Path)
	}
	return nil
}
