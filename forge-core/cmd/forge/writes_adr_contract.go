package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
)

type writesADRAttempt struct {
	target         string
	resolvedTarget string
	nextSequence   int
	baseline       map[string]string
}

func prepareWritesADRAttempt(root string, declared *asset.WritesADR) (*writesADRAttempt, error) {
	if declared == nil {
		return nil, nil
	}
	targetDir, target, err := containedADRTarget(root, declared.Target)
	if err != nil {
		return nil, err
	}
	resolved, err := resolveExistingPrefix(targetDir)
	if err != nil {
		return nil, fmt.Errorf("resolve writes_adr baseline target: %w", err)
	}
	baseline, err := snapshotADRTree(targetDir)
	if err != nil {
		return nil, err
	}
	return &writesADRAttempt{
		target: target, resolvedTarget: resolved,
		nextSequence: nextADRSequenceFromSnapshot(baseline), baseline: baseline,
	}, nil
}

func validateWritesADRAttempt(root string, attempt *writesADRAttempt) (string, error) {
	if attempt == nil {
		return "", nil
	}
	targetDir, target, err := containedADRTarget(root, attempt.target)
	if err != nil {
		return "", err
	}
	resolved, err := resolveExistingPrefix(targetDir)
	if err != nil || resolved != attempt.resolvedTarget || target != attempt.target {
		return "", fmt.Errorf("writes_adr target changed after build-time snapshot")
	}
	current, err := snapshotADRTree(targetDir)
	if err != nil {
		return "", err
	}
	added, altered := adrTreeDelta(attempt.baseline, current)
	if len(added) != 1 || len(altered) != 0 {
		return "", fmt.Errorf(
			"must create exactly one new ADR and leave the baseline unchanged (added=%v altered=%v)",
			added, altered,
		)
	}
	return validateADRCandidate(root, attempt, added[0])
}

func snapshotADRTree(dir string) (map[string]string, error) {
	info, err := os.Lstat(dir)
	if os.IsNotExist(err) {
		return map[string]string{}, nil
	}
	if err != nil {
		return nil, fmt.Errorf("inspect writes_adr target: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("writes_adr target must be a non-symlink directory")
	}
	snapshot := make(map[string]string)
	err = filepath.WalkDir(dir, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path == dir {
			return nil
		}
		rel, err := filepath.Rel(dir, path)
		if err != nil {
			return err
		}
		fingerprint, err := adrEntryFingerprint(path)
		if err != nil {
			return err
		}
		snapshot[filepath.ToSlash(rel)] = fingerprint
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("snapshot writes_adr target: %w", err)
	}
	return snapshot, nil
}

func adrEntryFingerprint(path string) (string, error) {
	info, err := os.Lstat(path)
	if err != nil {
		return "", err
	}
	switch {
	case info.Mode().IsRegular():
		data, err := os.ReadFile(path)
		if err != nil {
			return "", err
		}
		return fmt.Sprintf("file:%04o:%s", info.Mode().Perm(), artifact.Digest(data)), nil
	case info.IsDir():
		return fmt.Sprintf("dir:%04o", info.Mode().Perm()), nil
	case info.Mode()&os.ModeSymlink != 0:
		target, err := os.Readlink(path)
		if err != nil {
			return "", err
		}
		return "symlink:" + target, nil
	default:
		return "special:" + info.Mode().String(), nil
	}
}

func adrTreeDelta(before, after map[string]string) (added, altered []string) {
	for path, fingerprint := range after {
		previous, existed := before[path]
		if !existed {
			added = append(added, path)
		} else if previous != fingerprint {
			altered = append(altered, "changed:"+path)
		}
	}
	for path := range before {
		if _, exists := after[path]; !exists {
			altered = append(altered, "removed:"+path)
		}
	}
	sort.Strings(added)
	sort.Strings(altered)
	return added, altered
}

func validateADRCandidate(root string, attempt *writesADRAttempt, added string) (string, error) {
	if strings.Contains(added, "/") || !canonicalADRName(added, attempt.nextSequence) {
		return "", fmt.Errorf(
			"new ADR %q must match ADR-%04d-<title>.md in %s",
			added, attempt.nextSequence, attempt.target,
		)
	}
	relative := attempt.target + added
	full, normalized, err := containedRepoPath(root, relative)
	if err != nil || filepath.ToSlash(normalized) != relative {
		return "", fmt.Errorf("new ADR %q is not a normalized contained path", relative)
	}
	info, err := os.Lstat(full)
	if err != nil {
		return "", fmt.Errorf("inspect new ADR %q: %w", relative, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return "", fmt.Errorf("new ADR %q must be a non-symlink regular file", relative)
	}
	data, err := os.ReadFile(full)
	if err != nil {
		return "", fmt.Errorf("read new ADR %q: %w", relative, err)
	}
	if strings.TrimSpace(string(data)) == "" {
		return "", fmt.Errorf("new ADR %q must be non-empty", relative)
	}
	return relative, nil
}

func canonicalADRName(name string, sequence int) bool {
	prefix := fmt.Sprintf("ADR-%04d-", sequence)
	if !strings.HasPrefix(name, prefix) || !strings.HasSuffix(name, ".md") {
		return false
	}
	title := strings.TrimSuffix(strings.TrimPrefix(name, prefix), ".md")
	return title != "" && title != "." && !strings.Contains(title, "..") &&
		safeADRRelativePath(name)
}

func nextADRSequenceFromSnapshot(snapshot map[string]string) int {
	maximum := 0
	for name := range snapshot {
		if strings.Contains(name, "/") {
			continue
		}
		if number, ok := adrSequenceNumber(name); ok && number > maximum {
			maximum = number
		}
	}
	return maximum + 1
}

func adrSequenceNumber(name string) (int, bool) {
	if !strings.HasSuffix(name, ".md") {
		return 0, false
	}
	parts := strings.SplitN(strings.TrimPrefix(name, "ADR-"), "-", 2)
	if len(parts) != 2 {
		return 0, false
	}
	number, err := strconv.Atoi(parts[0])
	return number, err == nil
}
