package localcommandobservationproducer

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"forgeos/forge-core/internal/execbound"
)

const (
	maxSourceEntries       = 65_536
	maxSourceBytes         = int64(8 << 30)
	maxIndividualFileBytes = int64(1 << 30)
	maxGitOutputBytes      = 32 << 20
)

type inventoryRecord struct {
	indexMode string
	path      string
	tracking  string
}

// sourceSnapshot builds a hardened Git inventory over tracked and non-ignored
// untracked source, including docs/release and excluding only untracked .forge.
func sourceSnapshot(ctx context.Context, root string, childEnvironment []string) (SourceManifest, string, error) {
	canonicalRoot, gitPath, err := validateRepositoryRoot(ctx, root, childEnvironment)
	if err != nil {
		return SourceManifest{}, "", err
	}
	treeRoot, err := openSourceTreeRoot(canonicalRoot)
	if err != nil {
		return SourceManifest{}, "", err
	}
	defer func() { _ = treeRoot.handle.Close() }()
	revision, err := sourceRevision(ctx, canonicalRoot, gitPath, childEnvironment)
	if err != nil {
		return SourceManifest{}, "", err
	}
	records, err := sourceInventory(ctx, canonicalRoot, gitPath, childEnvironment)
	if err != nil {
		return SourceManifest{}, "", err
	}
	if err := treeRoot.verify(); err != nil {
		return SourceManifest{}, "", err
	}
	entries, err := inspectInventory(ctx, treeRoot, records)
	if err != nil {
		return SourceManifest{}, "", err
	}
	if err := treeRoot.verify(); err != nil {
		return SourceManifest{}, "", err
	}
	manifest := SourceManifest{
		APIVersion: SourceTreeAPIVersion, Canonicalization: Canonicalization,
		Entries: entries, ProfileID: sourceTreeProfileID, SourceRevision: revision,
	}
	_, digest, err := digestManifest(sourceDigestDomain, manifest)
	return manifest, digest, err
}

func validateRepositoryRoot(ctx context.Context, root string, environment []string) (string, string, error) {
	if err := ctx.Err(); err != nil {
		return "", "", fmt.Errorf("resolve repository root: %w", err)
	}
	absolute, err := filepath.Abs(root)
	if err != nil {
		return "", "", fmt.Errorf("resolve repository root: %w", err)
	}
	canonical, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", "", fmt.Errorf("resolve repository root symlinks: %w", err)
	}
	pathValue, ok := environmentStringValue(environment, "PATH")
	if !ok {
		return "", "", fmt.Errorf("child environment lacks PATH")
	}
	gitPath, err := resolveExecutable(ctx, "git", pathValue)
	if err != nil {
		return "", "", err
	}
	top, err := hardenedGitOutput(ctx, canonical, gitPath, environment, "rev-parse", "--show-toplevel")
	if err != nil {
		return "", "", fmt.Errorf("resolve Git worktree root: %w", err)
	}
	topPath, err := filepath.EvalSymlinks(strings.TrimSpace(string(top)))
	if err != nil || topPath != canonical {
		return "", "", fmt.Errorf("root %q is not the canonical Git toplevel", root)
	}
	return canonical, gitPath, nil
}

func sourceRevision(ctx context.Context, root, gitPath string, environment []string) (string, error) {
	formatRaw, err := hardenedGitOutput(ctx, root, gitPath, environment, "rev-parse", "--show-object-format")
	if err != nil {
		return "", fmt.Errorf("resolve Git object format: %w", err)
	}
	oidRaw, err := hardenedGitOutput(ctx, root, gitPath, environment, "rev-parse", "--verify", "HEAD")
	if err != nil {
		return "", fmt.Errorf("resolve source revision: %w", err)
	}
	format, oid := strings.TrimSpace(string(formatRaw)), strings.TrimSpace(string(oidRaw))
	if format != "sha1" && format != "sha256" {
		return "", fmt.Errorf("unsupported Git object format %q", format)
	}
	if len(oid) != map[string]int{"sha1": 40, "sha256": 64}[format] || !lowerHex(oid) {
		return "", fmt.Errorf("git HEAD is not a canonical %s object ID", format)
	}
	return "git-" + format + ":" + oid, nil
}

func sourceInventory(ctx context.Context, root, gitPath string, environment []string) ([]inventoryRecord, error) {
	tracked, err := hardenedGitOutput(ctx, root, gitPath, environment, "ls-files", "--cached", "--stage", "-z")
	if err != nil {
		return nil, fmt.Errorf("enumerate tracked source: %w", err)
	}
	records, seen, err := parseTrackedInventory(ctx, tracked)
	if err != nil {
		return nil, err
	}
	untracked, err := hardenedGitOutput(ctx, root, gitPath, environment, "ls-files", "--others", "--exclude-standard", "-z")
	if err != nil {
		return nil, fmt.Errorf("enumerate untracked source: %w", err)
	}
	for _, path := range splitNUL(untracked) {
		if err := ctx.Err(); err != nil {
			return nil, fmt.Errorf("enumerate untracked source: %w", err)
		}
		if err := validateInventoryPath(path, false); err != nil {
			return nil, err
		}
		if pathWithin(path, ".forge") {
			continue
		}
		if _, exists := seen[path]; !exists {
			records = append(records, inventoryRecord{path: path, tracking: "untracked"})
		}
	}
	if len(records) > maxSourceEntries {
		return nil, fmt.Errorf("source inventory exceeds %d entries", maxSourceEntries)
	}
	sort.Slice(records, func(i, j int) bool { return records[i].path < records[j].path })
	return records, nil
}

func parseTrackedInventory(ctx context.Context, raw []byte) ([]inventoryRecord, map[string]struct{}, error) {
	records := make([]inventoryRecord, 0)
	seen := make(map[string]struct{})
	for _, item := range splitNUL(raw) {
		if err := ctx.Err(); err != nil {
			return nil, nil, fmt.Errorf("parse tracked source inventory: %w", err)
		}
		tab := strings.IndexByte(item, '\t')
		if tab < 0 {
			return nil, nil, fmt.Errorf("malformed tracked source inventory")
		}
		fields, path := strings.Fields(item[:tab]), item[tab+1:]
		if len(fields) != 3 || fields[2] != "0" {
			return nil, nil, fmt.Errorf("tracked path %q has unresolved or malformed index stage", path)
		}
		if fields[0] == "160000" {
			return nil, nil, fmt.Errorf("tracked path %q is a forbidden gitlink", path)
		}
		if fields[0] != "100644" && fields[0] != "100755" && fields[0] != "120000" {
			return nil, nil, fmt.Errorf("tracked path %q has unsupported index mode %q", path, fields[0])
		}
		if err := validateInventoryPath(path, true); err != nil {
			return nil, nil, err
		}
		if _, exists := seen[path]; exists {
			return nil, nil, fmt.Errorf("duplicate tracked source path %q", path)
		}
		seen[path] = struct{}{}
		records = append(records, inventoryRecord{indexMode: fields[0], path: path, tracking: "tracked"})
	}
	return records, seen, nil
}

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

func sourceEntry(record inventoryRecord, kind string, count int64, digest *string, executable *bool, target *string) SourceEntry {
	var indexMode *string
	if record.indexMode != "" {
		indexMode = stringPointer(record.indexMode)
	}
	return SourceEntry{
		Bytes: count, ContentSHA256: digest, Executable: executable, IndexMode: indexMode,
		Kind: kind, Path: record.path, SymlinkTarget: target, Tracking: record.tracking,
	}
}

func validateInventoryPath(value string, tracked bool) error {
	if err := validateText("source path", value, false); err != nil {
		return err
	}
	hasDrivePrefix := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if filepath.IsAbs(value) || hasDrivePrefix || strings.Contains(value, `\`) ||
		filepath.ToSlash(filepath.Clean(value)) != value || value == "." {
		return fmt.Errorf("source path %q is not canonical repo-relative forward-slash form", value)
	}
	folded := foldASCII(value)
	if pathWithin(folded, ".git") || tracked && pathWithin(folded, ".forge") ||
		pathWithin(folded, ".forge") && !pathWithin(value, ".forge") {
		return fmt.Errorf("tracked or control source path %q is forbidden", value)
	}
	return nil
}

func hardenedGitOutput(ctx context.Context, root, gitPath string, environment []string, args ...string) ([]byte, error) {
	commandArgs := []string{
		"-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null",
		"-c", "core.excludesFile=/dev/null", "-c", "core.pager=cat", "--no-pager",
		"-C", root, "--work-tree=" + root,
	}
	commandArgs = append(commandArgs, args...)
	argv := append([]string{gitPath}, commandArgs...)
	result := execbound.Run(ctx, argv,
		execbound.Options{Timeout: 30 * time.Second, MaxOutputBytes: maxGitOutputBytes},
		execbound.CaptureSplit, execbound.Spec{Env: hardenedGitEnvironment(environment)})
	if ctxErr := ctx.Err(); ctxErr != nil {
		return nil, fmt.Errorf("hardened Git command canceled: %w", ctxErr)
	}
	if result.Err != nil {
		return nil, fmt.Errorf("hardened Git command failed: %w (%s)", result.Err, strings.TrimSpace(string(result.Stderr)))
	}
	if result.Total > maxGitOutputBytes || result.Total > int64(result.Retained) {
		return nil, fmt.Errorf("hardened Git output exceeds %d bytes", maxGitOutputBytes)
	}
	return append([]byte(nil), result.Stdout...), nil
}

func hardenedGitEnvironment(environment []string) []string {
	pathValue, _ := environmentStringValue(environment, "PATH")
	values := map[string]string{
		"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
		"GIT_OPTIONAL_LOCKS": "0", "GIT_PAGER": "cat", "GIT_TERMINAL_PROMPT": "0",
		"HOME": "/", "LANG": "C", "LC_ALL": "C", "PATH": pathValue,
	}
	if temporary, ok := environmentStringValue(environment, "TMPDIR"); ok {
		values["TMPDIR"] = temporary
	}
	keys := make([]string, 0, len(values))
	for name := range values {
		keys = append(keys, name)
	}
	sort.Strings(keys)
	result := make([]string, len(keys))
	for index, name := range keys {
		result[index] = name + "=" + values[name]
	}
	return result
}

func environmentStringValue(environment []string, name string) (string, bool) {
	for _, entry := range environment {
		entryName, value, ok := strings.Cut(entry, "=")
		if ok && entryName == name {
			return value, true
		}
	}
	return "", false
}

func splitNUL(value []byte) []string {
	result := make([]string, 0)
	for _, item := range bytes.Split(value, []byte{0}) {
		if len(item) != 0 {
			result = append(result, string(item))
		}
	}
	return result
}

func pathWithin(value, directory string) bool {
	return value == directory || strings.HasPrefix(value, directory+"/")
}

func lowerHex(value string) bool {
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return value != ""
}

func foldASCII(value string) string {
	folded := []byte(value)
	for index, character := range folded {
		if character >= 'A' && character <= 'Z' {
			folded[index] = character + ('a' - 'A')
		}
	}
	return string(folded)
}
