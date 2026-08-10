package gitworktreesource

import (
	"bytes"
	"context"
	"fmt"
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

// Capture records one bounded-interval Git inventory and its enumerated entry
// bytes. It is not an atomic filesystem snapshot or an execution-time pin.
func Capture(ctx context.Context, root string, environment []string) (Snapshot, error) {
	if _, err := exactPathValue(environment); err != nil {
		return Snapshot{}, err
	}
	canonicalRoot, gitPath, err := validateRepositoryRoot(ctx, root, environment)
	if err != nil {
		return Snapshot{}, err
	}
	treeRoot, err := openSourceTreeRoot(canonicalRoot)
	if err != nil {
		return Snapshot{}, err
	}
	defer func() { _ = treeRoot.handle.Close() }()
	revision, err := sourceRevision(ctx, canonicalRoot, gitPath, environment)
	if err != nil {
		return Snapshot{}, err
	}
	records, err := sourceInventory(ctx, canonicalRoot, gitPath, environment)
	if err != nil {
		return Snapshot{}, err
	}
	if err := treeRoot.verify(); err != nil {
		return Snapshot{}, err
	}
	entries, err := inspectInventory(ctx, treeRoot, records)
	if err != nil {
		return Snapshot{}, err
	}
	if err := treeRoot.verify(); err != nil {
		return Snapshot{}, err
	}
	manifest := SourceManifest{
		APIVersion: APIVersion, Canonicalization: Canonicalization,
		Entries: entries, ProfileID: ProfileID, SourceRevision: revision,
	}
	digest, err := Digest(manifest)
	if err != nil {
		return Snapshot{}, err
	}
	if err := Validate(manifest, digest); err != nil {
		return Snapshot{}, fmt.Errorf("validate captured source manifest: %w", err)
	}
	return Snapshot{
		Root: canonicalRoot, Manifest: manifest, SHA256: digest,
		captureManifestSHA256: digest, captureRootIdentity: treeRoot.identity,
	}, nil
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
	pathValue, err := exactPathValue(environment)
	if err != nil {
		return "", "", err
	}
	gitPath, err := resolveGitExecutable(ctx, pathValue)
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
	records, err = appendUntrackedInventory(ctx, untracked, records, seen)
	if err != nil {
		return nil, err
	}
	sort.Slice(records, func(i, j int) bool { return records[i].path < records[j].path })
	return records, nil
}

func parseTrackedInventory(ctx context.Context, raw []byte) ([]inventoryRecord, map[string]struct{}, error) {
	records := make([]inventoryRecord, 0)
	seen := make(map[string]struct{})
	err := forEachNUL(raw, func(item []byte) error {
		if err := ctx.Err(); err != nil {
			return fmt.Errorf("parse tracked source inventory: %w", err)
		}
		if len(records) >= maxSourceEntries {
			return fmt.Errorf("source inventory exceeds %d entries", maxSourceEntries)
		}
		record, err := parseTrackedInventoryItem(string(item))
		if err != nil {
			return err
		}
		if _, exists := seen[record.path]; exists {
			return fmt.Errorf("duplicate tracked source path %q", record.path)
		}
		seen[record.path] = struct{}{}
		records = append(records, record)
		return nil
	})
	if err != nil {
		return nil, nil, err
	}
	return records, seen, nil
}

func parseTrackedInventoryItem(item string) (inventoryRecord, error) {
	tab := strings.IndexByte(item, '\t')
	if tab < 0 {
		return inventoryRecord{}, fmt.Errorf("malformed tracked source inventory")
	}
	fields, path := strings.Fields(item[:tab]), item[tab+1:]
	if len(fields) != 3 || fields[2] != "0" {
		return inventoryRecord{}, fmt.Errorf(
			"tracked path %q has unresolved or malformed index stage", path)
	}
	if fields[0] == "160000" {
		return inventoryRecord{}, fmt.Errorf("tracked path %q is a forbidden gitlink", path)
	}
	if fields[0] != "100644" && fields[0] != "100755" && fields[0] != "120000" {
		return inventoryRecord{}, fmt.Errorf(
			"tracked path %q has unsupported index mode %q", path, fields[0])
	}
	if err := validateInventoryPath(path, true); err != nil {
		return inventoryRecord{}, err
	}
	return inventoryRecord{indexMode: fields[0], path: path, tracking: "tracked"}, nil
}

func appendUntrackedInventory(
	ctx context.Context,
	raw []byte,
	records []inventoryRecord,
	seen map[string]struct{},
) ([]inventoryRecord, error) {
	err := forEachNUL(raw, func(item []byte) error {
		if err := ctx.Err(); err != nil {
			return fmt.Errorf("enumerate untracked source: %w", err)
		}
		path := string(item)
		if err := validateInventoryPath(path, false); err != nil {
			return err
		}
		if pathWithin(path, ".forge") {
			return nil
		}
		if _, exists := seen[path]; exists {
			return nil
		}
		if len(records) >= maxSourceEntries {
			return fmt.Errorf("source inventory exceeds %d entries", maxSourceEntries)
		}
		seen[path] = struct{}{}
		records = append(records, inventoryRecord{path: path, tracking: "untracked"})
		return nil
	})
	return records, err
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
	pathValue, _ := exactPathValue(environment)
	values := map[string]string{
		"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
		"GIT_OPTIONAL_LOCKS": "0", "GIT_PAGER": "cat", "GIT_TERMINAL_PROMPT": "0",
		"HOME": "/", "LANG": "C", "LC_ALL": "C", "PATH": pathValue,
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

func forEachNUL(value []byte, visit func([]byte) error) error {
	for offset := 0; offset < len(value); {
		end := bytes.IndexByte(value[offset:], 0)
		if end < 0 {
			end = len(value) - offset
		}
		item := value[offset : offset+end]
		if len(item) != 0 {
			if err := visit(item); err != nil {
				return err
			}
		}
		offset += end + 1
	}
	return nil
}
