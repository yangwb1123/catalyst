package gitworktreesource

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
)

func inspectInventory(ctx context.Context, root *sourceTreeRoot, records []inventoryRecord) ([]SourceEntry, error) {
	entries := make([]SourceEntry, 0, len(records))
	var total int64
	for _, record := range records {
		if err := ctx.Err(); err != nil {
			return nil, fmt.Errorf("inspect source inventory: %w", err)
		}
		entry, err := inspectSourceEntry(ctx, root, record)
		if err != nil {
			return nil, err
		}
		if entry.Bytes > maxSourceBytes-total {
			return nil, fmt.Errorf("source inventory exceeds %d bytes", maxSourceBytes)
		}
		total += entry.Bytes
		entries = append(entries, entry)
	}
	return entries, nil
}

func inspectSourceEntry(ctx context.Context, root *sourceTreeRoot, record inventoryRecord) (SourceEntry, error) {
	parent, err := openSourceParent(root, record.path)
	if err != nil {
		return SourceEntry{}, err
	}
	defer parent.close()
	if parent.missing != nil {
		if record.tracking == "tracked" {
			if verifyErr := parent.verify(); verifyErr != nil {
				return SourceEntry{}, verifyErr
			}
			return sourceEntry(record, "deleted", 0, nil, nil, nil), nil
		}
		return SourceEntry{}, fmt.Errorf("inspect source path %q: parent %q disappeared", record.path, parent.missing.path)
	}
	info, err := parent.leafRoot.Lstat(parent.leaf)
	if os.IsNotExist(err) && record.tracking == "tracked" {
		if verifyErr := parent.verifyMissingLeaf(); verifyErr != nil {
			return SourceEntry{}, verifyErr
		}
		return sourceEntry(record, "deleted", 0, nil, nil, nil), nil
	}
	if err != nil {
		return SourceEntry{}, fmt.Errorf("inspect source path %q: %w", record.path, err)
	}
	if err := parent.verify(); err != nil {
		return SourceEntry{}, err
	}
	if info.Mode().IsRegular() {
		return inspectRegularSourceEntry(ctx, parent, record, info)
	}
	if info.Mode()&os.ModeSymlink != 0 {
		return inspectSymlinkSourceEntry(parent, record, info)
	}
	return SourceEntry{}, fmt.Errorf("source path %q has unsupported file type %s", record.path, info.Mode().Type())
}

func inspectRegularSourceEntry(
	ctx context.Context,
	parent *sourceParent,
	record inventoryRecord,
	info os.FileInfo,
) (SourceEntry, error) {
	if record.tracking == "tracked" && record.indexMode == "120000" {
		return SourceEntry{}, fmt.Errorf(
			"tracked source path %q changed from index symlink to working-tree regular file",
			record.path,
		)
	}
	file, opened, err := parent.openRegular(info)
	if err != nil {
		return SourceEntry{}, err
	}
	defer func() { _ = file.Close() }()
	digest, count, err := hashOpenedSourceFile(ctx, record.path, file, opened)
	if err != nil {
		return SourceEntry{}, err
	}
	if err := parent.verifyRegularLeaf(opened); err != nil {
		return SourceEntry{}, err
	}
	executable := opened.Mode().Perm()&0o111 != 0
	return sourceEntry(record, "regular", count, stringPointer(digest), boolPointer(executable), nil), nil
}

func inspectSymlinkSourceEntry(
	parent *sourceParent,
	record inventoryRecord,
	info os.FileInfo,
) (SourceEntry, error) {
	if record.tracking == "tracked" && record.indexMode != "120000" {
		return SourceEntry{}, fmt.Errorf(
			"tracked source path %q changed from index regular file to working-tree symlink",
			record.path,
		)
	}
	target, err := parent.leafRoot.Readlink(parent.leaf)
	if err != nil {
		return SourceEntry{}, fmt.Errorf("read source symlink %q: %w", record.path, err)
	}
	if err := parent.verifySymlinkLeaf(info); err != nil {
		return SourceEntry{}, err
	}
	if err := validateText("symlink target", target, false); err != nil {
		return SourceEntry{}, fmt.Errorf("source symlink %q: %w", record.path, err)
	}
	digest := sha256Bytes([]byte(target))
	return sourceEntry(
		record, "symlink", int64(len([]byte(target))), stringPointer(digest),
		boolPointer(false), stringPointer(target),
	), nil
}

func sourceEntry(
	record inventoryRecord,
	kind string,
	count int64,
	digest *string,
	executable *bool,
	target *string,
) SourceEntry {
	var indexMode *string
	if record.indexMode != "" {
		indexMode = stringPointer(record.indexMode)
	}
	return SourceEntry{
		Bytes: count, ContentSHA256: digest, Executable: executable, IndexMode: indexMode,
		Kind: kind, Path: record.path, SymlinkTarget: target, Tracking: record.tracking,
	}
}

func hashOpenedSourceFile(ctx context.Context, path string, file *os.File, before os.FileInfo) (string, int64, error) {
	if before.Size() > maxIndividualFileBytes {
		return "", 0, fmt.Errorf("file %q exceeds %d bytes", path, maxIndividualFileBytes)
	}
	digest, count, err := hashReaderWithLimit(ctx, path, file, maxIndividualFileBytes)
	if err != nil {
		return "", 0, err
	}
	after, err := file.Stat()
	if err != nil {
		return "", 0, fmt.Errorf("stat source path %q after hashing: %w", path, err)
	}
	if count != before.Size() || !stableSourceFile(before, after) {
		return "", 0, fmt.Errorf("source path %q changed while hashing", path)
	}
	return digest, count, nil
}

func hashReaderWithLimit(ctx context.Context, label string, reader io.Reader, limit int64) (string, int64, error) {
	hasher, buffer := sha256.New(), make([]byte, 128<<10)
	var count int64
	for {
		if err := ctx.Err(); err != nil {
			return "", 0, fmt.Errorf("hash %q: %w", label, err)
		}
		read, readErr := reader.Read(buffer)
		if read > 0 {
			if int64(read) > limit-count {
				return "", 0, fmt.Errorf("file %q exceeds %d bytes", label, limit)
			}
			_, _ = hasher.Write(buffer[:read])
			count += int64(read)
		}
		if readErr == io.EOF {
			return hex.EncodeToString(hasher.Sum(nil)), count, nil
		}
		if readErr != nil {
			return "", 0, fmt.Errorf("hash %q: %w", label, readErr)
		}
	}
}
