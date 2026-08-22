package projectsnapshot

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"sort"
)

const (
	observeBeforeClassification = "before-classification"
	observeBeforeLeafLstat      = "before-leaf-lstat"
	observeBeforeRegularOpen    = "before-regular-open"
	observeBeforeRegularHash    = "before-regular-hash"
	observeAfterRegularContent  = "after-regular-content"
	observeAfterFullPass        = "after-full-pass"
	observeBeforeAnchorOpen     = "before-anchor-component-open"
	observeBeforeGitOpen        = "before-git-executable-open"
)

type captureObserver func(stage, path string)

type inspection struct {
	counts     CoverageCounts
	entries    []Entry
	exclusions []Exclusion
}

func inspectRecords(
	ctx context.Context,
	root *treeRoot,
	inventory gitInventory,
	observer captureObserver,
) (inspection, error) {
	result := inspection{entries: []Entry{}, exclusions: []Exclusion{}}
	result.counts = CoverageCounts{
		IgnoredPathCount: inventory.ignored, TrackedCount: inventory.tracked,
		UniverseCount: int64(len(inventory.records)), UntrackedCount: inventory.untracked,
	}
	var total int64
	for _, record := range inventory.records {
		entry, exclusion, err := inspectRecord(ctx, root, record, observer)
		if err != nil {
			return inspection{}, err
		}
		if exclusion != nil {
			if err := result.appendExclusion(*exclusion); err != nil {
				return inspection{}, err
			}
			continue
		}
		if entry.Bytes > maxTotalBytes-total {
			return inspection{}, fmt.Errorf("included source bytes exceed %d", maxTotalBytes)
		}
		total += entry.Bytes
		if entry.Kind == "regular" {
			result.counts.IncludedRegularCount++
		} else {
			result.counts.TrackedAbsentCount++
		}
		result.entries = append(result.entries, entry)
	}
	sort.Slice(result.entries, func(i, j int) bool {
		return result.entries[i].PathSHA256 < result.entries[j].PathSHA256
	})
	sort.Slice(result.exclusions, func(i, j int) bool {
		return result.exclusions[i].PathSHA256 < result.exclusions[j].PathSHA256
	})
	if err := validateUniquePathDigests(result); err != nil {
		return inspection{}, err
	}
	return result, root.verify()
}

func inspectRecord(
	ctx context.Context,
	root *treeRoot,
	record inventoryRecord,
	observer captureObserver,
) (Entry, *Exclusion, error) {
	observe(observer, observeBeforeClassification, record.path)
	if reason := protectedPathReason(record.path); reason != "" {
		return Entry{}, newExclusion(record, reason, false), nil
	}
	parent, err := openParent(ctx, root, record.path)
	if err != nil {
		return Entry{}, nil, err
	}
	defer parent.close()
	if parent.missing != nil {
		if record.tracking != "tracked" {
			return Entry{}, nil, fmt.Errorf("untracked project source parent disappeared")
		}
		if err := parent.verify(); err != nil {
			return Entry{}, nil, err
		}
		return newAbsentEntry(record), nil, nil
	}
	observe(observer, observeBeforeLeafLstat, record.path)
	info, err := parent.leafRoot.Lstat(parent.leaf)
	if os.IsNotExist(err) && record.tracking == "tracked" {
		if verifyErr := parent.verify(); verifyErr != nil {
			return Entry{}, nil, verifyErr
		}
		return newAbsentEntry(record), nil, nil
	}
	if err != nil {
		return Entry{}, nil, fmt.Errorf("inspect project source leaf: %w", err)
	}
	if err := parent.verify(); err != nil {
		return Entry{}, nil, err
	}
	if info.Mode()&os.ModeSymlink != 0 {
		return Entry{}, newExclusion(record, "symlink_leaf", true), nil
	}
	if !info.Mode().IsRegular() {
		return Entry{}, nil, fmt.Errorf("project source leaf has an unsupported file type")
	}
	if record.tracking == "tracked" && record.indexMode == "120000" {
		return Entry{}, nil, fmt.Errorf("tracked symlink changed to a regular file")
	}
	entry, err := inspectRegular(ctx, parent, record, info, observer)
	return entry, nil, err
}

func inspectRegular(
	ctx context.Context,
	parent *sourceParent,
	record inventoryRecord,
	before os.FileInfo,
	observer captureObserver,
) (Entry, error) {
	if before.Size() < 0 || before.Size() > maxFileBytes {
		return Entry{}, fmt.Errorf("project source leaf exceeds %d bytes", maxFileBytes)
	}
	if err := requireSingleLink(nil, before); err != nil {
		return Entry{}, err
	}
	observe(observer, observeBeforeRegularOpen, record.path)
	file, opened, err := parent.openRegular(before)
	if err != nil {
		return Entry{}, err
	}
	defer func() { _ = file.Close() }()
	observe(observer, observeBeforeRegularHash, record.path)
	digest, count, err := hashRegular(ctx, file, record.path)
	if err != nil {
		return Entry{}, err
	}
	observe(observer, observeAfterRegularContent, record.path)
	if count != opened.Size() {
		return Entry{}, fmt.Errorf("project source leaf changed while hashing")
	}
	if err := parent.verifyRegular(file, opened); err != nil {
		return Entry{}, err
	}
	executable := opened.Mode().Perm()&0o111 != 0
	entry := Entry{
		Bytes: count, ContentSHA256: stringPointer(digest), Executable: boolPointer(executable),
		IndexMode: optionalIndexMode(record), Kind: "regular", Path: record.path,
		PathSHA256: pathDigest(record.path), Tracking: record.tracking,
	}
	entry.EntrySHA256, err = domainDigest(entryDomain, entry, maxManifestBytes)
	return entry, err
}

func hashRegular(ctx context.Context, file *os.File, path string) (string, int64, error) {
	hasher, buffer := sha256.New(), make([]byte, 128<<10)
	var count int64
	for {
		if err := ctx.Err(); err != nil {
			return "", 0, fmt.Errorf("hash project source path: %w", err)
		}
		read, readErr := file.Read(buffer)
		if read > 0 {
			if int64(read) > maxFileBytes-count {
				return "", 0, fmt.Errorf("project source leaf exceeds %d bytes", maxFileBytes)
			}
			_, _ = hasher.Write(buffer[:read])
			count += int64(read)
		}
		if readErr == io.EOF {
			return hex.EncodeToString(hasher.Sum(nil)), count, nil
		}
		if readErr != nil {
			return "", 0, fmt.Errorf("hash project source path %q: %w", path, readErr)
		}
	}
}

func newAbsentEntry(record inventoryRecord) Entry {
	entry := Entry{
		Bytes: 0, ContentSHA256: nil, EntrySHA256: "", Executable: nil,
		IndexMode: optionalIndexMode(record), Kind: "tracked_absent", Path: record.path,
		PathSHA256: pathDigest(record.path), Tracking: record.tracking,
	}
	entry.EntrySHA256, _ = domainDigest(entryDomain, entry, maxManifestBytes)
	return entry
}

func newExclusion(record inventoryRecord, reason string, observed bool) *Exclusion {
	value := &Exclusion{
		IndexMode: optionalIndexMode(record), LeafFilesystemObserved: observed,
		PathSHA256: pathDigest(record.path), Reason: reason, Tracking: record.tracking,
	}
	value.ExclusionSHA256, _ = domainDigest(exclusionDomain, *value, maxManifestBytes)
	return value
}

func optionalIndexMode(record inventoryRecord) *string {
	if record.indexMode == "" {
		return nil
	}
	return stringPointer(record.indexMode)
}

func (value *inspection) appendExclusion(exclusion Exclusion) error {
	if len(value.exclusions) >= maxExcludedEntries {
		return fmt.Errorf("excluded source paths exceed %d", maxExcludedEntries)
	}
	switch exclusion.Reason {
	case "control_path":
		value.counts.ExcludedControlCount++
	case "sensitive_path":
		value.counts.ExcludedSensitiveCount++
	case "symlink_leaf":
		value.counts.ExcludedSymlinkCount++
	default:
		return fmt.Errorf("unknown project snapshot exclusion reason")
	}
	value.exclusions = append(value.exclusions, exclusion)
	return nil
}

func validateUniquePathDigests(value inspection) error {
	seen := make(map[string]struct{}, len(value.entries)+len(value.exclusions))
	for _, entry := range value.entries {
		if _, exists := seen[entry.PathSHA256]; exists {
			return fmt.Errorf("project path digest collision")
		}
		seen[entry.PathSHA256] = struct{}{}
	}
	for _, exclusion := range value.exclusions {
		if _, exists := seen[exclusion.PathSHA256]; exists {
			return fmt.Errorf("project path digest collision")
		}
		seen[exclusion.PathSHA256] = struct{}{}
	}
	return nil
}

func observe(observer captureObserver, stage, path string) {
	if observer != nil {
		observer(stage, path)
	}
}
