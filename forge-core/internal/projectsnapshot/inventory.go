package projectsnapshot

import (
	"bytes"
	"context"
	"fmt"
	"sort"
	"strings"
	"unicode/utf8"
)

type inventoryRecord struct {
	indexMode string
	path      string
	tracking  string
}

type gitInventory struct {
	ignored   int64
	records   []inventoryRecord
	revision  string
	tracked   int64
	untracked int64
	version   string
}

func enumerateInventory(
	ctx context.Context,
	root *gitRepositoryRoot,
	git *observedGit,
	environment []string,
) (gitInventory, error) {
	revision, err := readSourceRevision(ctx, root, git, environment)
	if err != nil {
		return gitInventory{}, err
	}
	version, err := readGitVersion(ctx, git, environment)
	if err != nil {
		return gitInventory{}, err
	}
	records, seen, err := enumerateTrackedInventory(ctx, root, git, environment, revision)
	if err != nil {
		return gitInventory{}, err
	}
	untrackedRaw, err := gitOutput(ctx, root, git, environment, "ls-files", "--others", "--exclude-standard", "-z")
	if err != nil {
		return gitInventory{}, fmt.Errorf("enumerate untracked paths: %w", err)
	}
	records, untracked, err := appendUntracked(ctx, untrackedRaw, records, seen)
	if err != nil {
		return gitInventory{}, err
	}
	ignoredRaw, err := gitOutput(ctx, root, git, environment,
		"ls-files", "--others", "--ignored", "--exclude-standard", "-z")
	if err != nil {
		return gitInventory{}, fmt.Errorf("count ignored paths: %w", err)
	}
	ignored, err := countIgnored(ctx, ignoredRaw, seen)
	if err != nil {
		return gitInventory{}, err
	}
	sort.Slice(records, func(first, second int) bool { return records[first].path < records[second].path })
	return gitInventory{
		ignored: ignored, records: records, revision: revision,
		tracked: int64(len(records)) - untracked, untracked: untracked, version: version,
	}, nil
}

func enumerateTrackedInventory(
	ctx context.Context,
	root *gitRepositoryRoot,
	git *observedGit,
	environment []string,
	revision string,
) ([]inventoryRecord, map[string]struct{}, error) {
	raw, err := gitOutput(ctx, root, git, environment,
		"ls-files", "--cached", "--stage", "-z")
	if err != nil {
		return nil, nil, fmt.Errorf("enumerate tracked paths: %w", err)
	}
	records, seen, err := parseTracked(ctx, raw, sourceOIDLength(revision))
	if err != nil {
		return nil, nil, err
	}
	raw, err = gitOutput(ctx, root, git, environment,
		"ls-files", "--cached", "--debug", "-z")
	if err != nil {
		return nil, nil, fmt.Errorf("enumerate tracked index flags: %w", err)
	}
	if err := validateTrackedIndexDebug(raw, seen); err != nil {
		return nil, nil, err
	}
	return records, seen, nil
}

func readSourceRevision(
	ctx context.Context,
	root *gitRepositoryRoot,
	git *observedGit,
	environment []string,
) (string, error) {
	formatRaw, err := gitOutput(ctx, root, git, environment, "rev-parse", "--show-object-format")
	if err != nil {
		return "", fmt.Errorf("resolve Git object format: %w", err)
	}
	oidRaw, err := gitOutput(ctx, root, git, environment, "rev-parse", "--verify", "HEAD")
	if err != nil {
		return "", fmt.Errorf("resolve Git HEAD: %w", err)
	}
	format, formatErr := exactGitLine(formatRaw, maxShortTextBytes)
	oid, oidErr := exactGitLine(oidRaw, maxShortTextBytes)
	if formatErr != nil || oidErr != nil {
		return "", fmt.Errorf("Git HEAD object identity is invalid")
	}
	want, supported := map[string]int{"sha1": 40, "sha256": 64}[format]
	if !supported || len(oid) != want || !lowerHex(oid) {
		return "", fmt.Errorf("Git HEAD object identity is invalid")
	}
	return "git-" + format + ":" + oid, nil
}

func readGitVersion(ctx context.Context, git *observedGit, environment []string) (string, error) {
	raw, err := gitOutput(ctx, nil, git, environment, "version")
	if err != nil {
		return "", fmt.Errorf("Git version output is invalid")
	}
	value, lineErr := exactGitLine(raw, maxShortTextBytes)
	if !strings.HasPrefix(value, "git version ") ||
		lineErr != nil || !validBoundedText(value, maxShortTextBytes) {
		return "", fmt.Errorf("Git version output is invalid")
	}
	return value, nil
}

func exactGitLine(raw []byte, maximum int) (string, error) {
	if !utf8.Valid(raw) || len(raw) < 2 || raw[len(raw)-1] != '\n' ||
		bytes.Count(raw, []byte{'\n'}) != 1 {
		return "", fmt.Errorf("Git line output has invalid framing")
	}
	value := string(raw[:len(raw)-1])
	if strings.TrimSpace(value) != value || !validBoundedText(value, maximum) {
		return "", fmt.Errorf("Git line output has invalid text")
	}
	return value, nil
}

func parseTracked(
	ctx context.Context,
	raw []byte,
	oidLength int,
) ([]inventoryRecord, map[string]struct{}, error) {
	records := make([]inventoryRecord, 0)
	seen := make(map[string]struct{})
	err := forEachNUL(raw, func(item []byte) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		record, err := parseTrackedItem(string(item), oidLength)
		if err != nil {
			return err
		}
		if _, duplicate := seen[record.path]; duplicate {
			return fmt.Errorf("tracked inventory contains a duplicate or unresolved path")
		}
		if len(records) >= maxUniverseEntries {
			return fmt.Errorf("source universe exceeds %d entries", maxUniverseEntries)
		}
		seen[record.path] = struct{}{}
		records = append(records, record)
		return nil
	})
	return records, seen, err
}

func parseTrackedItem(item string, oidLength int) (inventoryRecord, error) {
	tab := strings.IndexByte(item, '\t')
	if tab < 0 {
		return inventoryRecord{}, fmt.Errorf("tracked inventory item is malformed")
	}
	fields, path := strings.Split(item[:tab], " "), item[tab+1:]
	if len(fields) != 3 || fields[2] != "0" {
		return inventoryRecord{}, fmt.Errorf("tracked path has a nonzero or malformed stage")
	}
	if len(fields[1]) != oidLength || !lowerHex(fields[1]) {
		return inventoryRecord{}, fmt.Errorf("tracked path has a malformed object identity")
	}
	if fields[0] == "160000" {
		return inventoryRecord{}, fmt.Errorf("tracked path is a forbidden gitlink")
	}
	if fields[0] != "100644" && fields[0] != "100755" && fields[0] != "120000" {
		return inventoryRecord{}, fmt.Errorf("tracked path has an unsupported index mode")
	}
	if err := validateInventoryPath(path); err != nil {
		return inventoryRecord{}, err
	}
	return inventoryRecord{indexMode: fields[0], path: path, tracking: "tracked"}, nil
}

func appendUntracked(
	ctx context.Context,
	raw []byte,
	records []inventoryRecord,
	seen map[string]struct{},
) ([]inventoryRecord, int64, error) {
	var count int64
	err := forEachNUL(raw, func(item []byte) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		path := string(item)
		if err := validateInventoryPath(path); err != nil {
			return err
		}
		if _, duplicate := seen[path]; duplicate {
			return fmt.Errorf("untracked inventory overlaps another path")
		}
		if len(records) >= maxUniverseEntries {
			return fmt.Errorf("source universe exceeds %d entries", maxUniverseEntries)
		}
		seen[path] = struct{}{}
		records = append(records, inventoryRecord{path: path, tracking: "untracked"})
		count++
		return nil
	})
	return records, count, err
}

func countIgnored(ctx context.Context, raw []byte, universe map[string]struct{}) (int64, error) {
	return countIgnoredBounded(ctx, raw, universe, maxIgnoredPaths)
}

func countIgnoredBounded(
	ctx context.Context,
	raw []byte,
	universe map[string]struct{},
	maximum int64,
) (int64, error) {
	var count int64
	seen := make(map[string]struct{})
	err := forEachNUL(raw, func(item []byte) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		path := string(item)
		if err := validateInventoryPath(path); err != nil {
			return err
		}
		if _, duplicate := seen[path]; duplicate {
			return fmt.Errorf("ignored inventory contains a duplicate path")
		}
		if _, overlap := universe[path]; overlap {
			return fmt.Errorf("ignored inventory overlaps the source universe")
		}
		seen[path] = struct{}{}
		count++
		if count > maximum {
			return fmt.Errorf("ignored path count exceeds %d", maximum)
		}
		return nil
	})
	return count, err
}

func sourceOIDLength(revision string) int {
	if strings.HasPrefix(revision, "git-sha256:") {
		return 64
	}
	return 40
}

func forEachNUL(raw []byte, visit func([]byte) error) error {
	if len(raw) != 0 && raw[len(raw)-1] != 0 {
		return fmt.Errorf("Git inventory is not NUL terminated")
	}
	for offset := 0; offset < len(raw); {
		end := bytes.IndexByte(raw[offset:], 0)
		if end < 0 {
			return fmt.Errorf("Git inventory is malformed")
		}
		item := raw[offset : offset+end]
		if len(item) == 0 {
			return fmt.Errorf("Git inventory contains an empty record")
		}
		if err := visit(item); err != nil {
			return err
		}
		offset += end + 1
	}
	return nil
}
