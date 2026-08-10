package gitworktreesource

import (
	"context"
	"fmt"
	"io"
	"os"
	"sort"
	"strings"
)

const (
	regularReadBeforeFile     = "before-file"
	regularReadAfterLeafLstat = "after-leaf-lstat"
	regularReadAfterContent   = "after-content"
)

type regularReadObserver func(stage, path string)

// ReadRegularFiles reads a strictly ordered set of regular source paths and
// requires every returned byte sequence to match snapshot.Manifest exactly.
func ReadRegularFiles(
	ctx context.Context,
	snapshot Snapshot,
	paths []string,
	limits RegularReadLimits,
) ([]RegularFile, error) {
	return readRegularFilesWith(ctx, snapshot, paths, limits, nil)
}

func readRegularFilesWith(
	ctx context.Context,
	snapshot Snapshot,
	paths []string,
	limits RegularReadLimits,
	observer regularReadObserver,
) ([]RegularFile, error) {
	entries, err := validateRegularRead(snapshot, paths, limits)
	if err != nil {
		return nil, err
	}
	if err := ctx.Err(); err != nil {
		return nil, fmt.Errorf("read regular source files: %w", err)
	}
	root, err := openSourceTreeRoot(snapshot.Root)
	if err != nil {
		return nil, err
	}
	defer func() { _ = root.handle.Close() }()
	if !stableSourceDirectory(snapshot.captureRootIdentity, root.identity) {
		return nil, fmt.Errorf("canonical repository root %q differs from captured source root", snapshot.Root)
	}
	if len(entries) == 0 {
		return []RegularFile{}, root.verify()
	}
	return readRegularEntries(ctx, root, entries, limits.MaxFileBytes, observer)
}

func validateRegularRead(
	snapshot Snapshot,
	paths []string,
	limits RegularReadLimits,
) ([]SourceEntry, error) {
	if err := validateRegularReadLimits(limits); err != nil {
		return nil, err
	}
	if err := Validate(snapshot.Manifest, snapshot.SHA256); err != nil {
		return nil, fmt.Errorf("validate source snapshot: %w", err)
	}
	if snapshot.captureManifestSHA256 == "" || snapshot.SHA256 != snapshot.captureManifestSHA256 {
		return nil, fmt.Errorf("source snapshot does not match its captured manifest seal")
	}
	if snapshot.Root == "" || snapshot.captureRootIdentity == nil || paths == nil ||
		len(paths) > limits.MaxFiles {
		return nil, fmt.Errorf("regular source path set is invalid or exceeds %d files", limits.MaxFiles)
	}
	entries := make([]SourceEntry, len(paths))
	var total int64
	for index, sourcePath := range paths {
		entry, err := selectRegularEntry(snapshot.Manifest, paths, index, limits.MaxPathDepth)
		if err != nil {
			return nil, err
		}
		if entry.Bytes > limits.MaxFileBytes || entry.Bytes > limits.MaxTotalBytes-total {
			return nil, fmt.Errorf("regular source path %q exceeds read limits", sourcePath)
		}
		total += entry.Bytes
		entries[index] = entry
	}
	return entries, nil
}

func validateRegularReadLimits(limits RegularReadLimits) error {
	if limits.MaxFiles < 1 || limits.MaxFiles > maxSourceEntries || limits.MaxFileBytes < 1 ||
		limits.MaxFileBytes > maxIndividualFileBytes || limits.MaxTotalBytes < 1 ||
		limits.MaxTotalBytes > maxSourceBytes || limits.MaxPathDepth < 1 ||
		limits.MaxPathDepth > maxTextScalars {
		return fmt.Errorf("regular source read limits are invalid")
	}
	return nil
}

func selectRegularEntry(
	manifest SourceManifest,
	paths []string,
	index, maxDepth int,
) (SourceEntry, error) {
	sourcePath := paths[index]
	if index > 0 && paths[index-1] >= sourcePath {
		return SourceEntry{}, fmt.Errorf("regular source paths must be strictly sorted and unique")
	}
	if err := validateInventoryPath(sourcePath, false); err != nil {
		return SourceEntry{}, err
	}
	if strings.Count(sourcePath, "/")+1 > maxDepth {
		return SourceEntry{}, fmt.Errorf("regular source path %q exceeds path-depth limit", sourcePath)
	}
	position := sort.Search(len(manifest.Entries), func(candidate int) bool {
		return manifest.Entries[candidate].Path >= sourcePath
	})
	if position == len(manifest.Entries) || manifest.Entries[position].Path != sourcePath {
		return SourceEntry{}, fmt.Errorf("regular source path %q is absent from source manifest", sourcePath)
	}
	entry := manifest.Entries[position]
	if entry.Kind != "regular" || entry.ContentSHA256 == nil {
		return SourceEntry{}, fmt.Errorf("source path %q is not a manifest-bound regular file", sourcePath)
	}
	return entry, nil
}

func readRegularEntries(
	ctx context.Context,
	root *sourceTreeRoot,
	entries []SourceEntry,
	maxFileBytes int64,
	observer regularReadObserver,
) ([]RegularFile, error) {
	result := make([]RegularFile, 0, len(entries))
	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return nil, fmt.Errorf("read regular source files: %w", err)
		}
		observeRegularRead(observer, regularReadBeforeFile, entry.Path)
		value, err := readRegularEntry(ctx, root, entry, maxFileBytes, observer)
		if err != nil {
			return nil, err
		}
		result = append(result, value)
	}
	if err := root.verify(); err != nil {
		return nil, err
	}
	return cloneRegularFiles(result), nil
}

func readRegularEntry(
	ctx context.Context,
	root *sourceTreeRoot,
	entry SourceEntry,
	maxFileBytes int64,
	observer regularReadObserver,
) (RegularFile, error) {
	parent, err := openSourceParent(ctx, root, entry.Path)
	if err != nil {
		return RegularFile{}, err
	}
	defer parent.close()
	before, err := parent.leafRoot.Lstat(parent.leaf)
	if err != nil || !before.Mode().IsRegular() {
		return RegularFile{}, fmt.Errorf("source path %q is not an available regular file", entry.Path)
	}
	observeRegularRead(observer, regularReadAfterLeafLstat, entry.Path)
	file, opened, err := parent.openRegular(before)
	if err != nil {
		return RegularFile{}, err
	}
	defer func() { _ = file.Close() }()
	content, err := readRegularContent(ctx, entry.Path, file, maxFileBytes)
	if err != nil {
		return RegularFile{}, err
	}
	observeRegularRead(observer, regularReadAfterContent, entry.Path)
	if err := verifyRegularContent(parent, file, opened, entry, content); err != nil {
		return RegularFile{}, err
	}
	return RegularFile{Content: content, Path: entry.Path, SHA256: *entry.ContentSHA256}, nil
}

func readRegularContent(ctx context.Context, sourcePath string, file *os.File, limit int64) ([]byte, error) {
	reader := &regularContextReader{ctx: ctx, reader: io.LimitReader(file, limit+1)}
	content, err := io.ReadAll(reader)
	if err != nil {
		return nil, fmt.Errorf("read regular source path %q: %w", sourcePath, err)
	}
	if int64(len(content)) > limit {
		return nil, fmt.Errorf("regular source path %q exceeds %d bytes", sourcePath, limit)
	}
	return content, nil
}

func verifyRegularContent(
	parent *sourceParent,
	file *os.File,
	opened os.FileInfo,
	entry SourceEntry,
	content []byte,
) error {
	after, statErr := file.Stat()
	if statErr != nil || int64(len(content)) != entry.Bytes || !stableSourceFile(opened, after) {
		return fmt.Errorf("source path %q changed while reading", entry.Path)
	}
	if err := parent.verifyRegularLeaf(opened); err != nil {
		return err
	}
	if sha256Bytes(content) != *entry.ContentSHA256 {
		return fmt.Errorf("source path %q content does not match source manifest", entry.Path)
	}
	return nil
}

type regularContextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (reader *regularContextReader) Read(buffer []byte) (int, error) {
	if err := reader.ctx.Err(); err != nil {
		return 0, err
	}
	return reader.reader.Read(buffer)
}

func observeRegularRead(observer regularReadObserver, stage, path string) {
	if observer != nil {
		observer(stage, path)
	}
}
