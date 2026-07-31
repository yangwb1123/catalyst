package evolvescan

import (
	"bufio"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	maxEvidenceFileBytes = 1 << 20
	maxEvidenceLineBytes = 1 << 20
)

func validateEvidencePath(root, name string, line int) error {
	if err := validateEvidencePathText(name); err != nil {
		return err
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return fmt.Errorf("resolve repository root: %w", err)
	}
	rootInfo, err := os.Lstat(rootAbs)
	if err != nil || !rootInfo.IsDir() || rootInfo.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("repository root must be an available non-symlink directory")
	}
	current := rootAbs
	parts := strings.Split(name, "/")
	var finalInfo os.FileInfo
	for i, part := range parts {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err != nil {
			return fmt.Errorf("path %q is unavailable: %w", name, err)
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("path %q traverses a symlink", name)
		}
		if i < len(parts)-1 && !info.IsDir() {
			return fmt.Errorf("path %q has a non-directory prefix", name)
		}
		finalInfo = info
	}
	if finalInfo == nil || !finalInfo.Mode().IsRegular() {
		return fmt.Errorf("path %q is not a regular file", name)
	}
	rel, err := filepath.Rel(rootAbs, current)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return fmt.Errorf("path %q escapes repository", name)
	}
	return validateOpenedEvidence(current, name, line, finalInfo)
}

func validateEvidencePathText(name string) error {
	if name == "" || !utf8.ValidString(name) || strings.Contains(name, `\`) ||
		strings.ContainsRune(name, 0) {
		return fmt.Errorf("path %q must be a non-empty UTF-8 forward-slash repository path", name)
	}
	for _, r := range name {
		if unicode.IsControl(r) {
			return fmt.Errorf("path %q must not contain control characters", name)
		}
	}
	if path.IsAbs(name) || path.Clean(name) != name || name == "." ||
		name == ".." || strings.HasPrefix(name, "../") {
		return fmt.Errorf("path %q is not canonical repository-relative", name)
	}
	first := strings.SplitN(name, "/", 2)[0]
	if first == ".git" || first == ".forge" {
		return fmt.Errorf("path %q points into protected repository control state", name)
	}
	return nil
}

func validateOpenedEvidence(full, name string, line int, expected os.FileInfo) error {
	file, err := os.Open(full)
	if err != nil {
		return fmt.Errorf("path %q is not readable: %w", name, err)
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		return fmt.Errorf("path %q cannot be inspected: %w", name, err)
	}
	if !info.Mode().IsRegular() || !os.SameFile(expected, info) {
		return fmt.Errorf("path %q changed identity while being validated", name)
	}
	if info.Size() <= 0 {
		return fmt.Errorf("path %q is empty and cannot locate evidence", name)
	}
	if info.Size() > maxEvidenceFileBytes {
		return fmt.Errorf("path %q exceeds the %d-byte evidence read limit", name, maxEvidenceFileBytes)
	}
	return validateEvidenceLine(file, name, line)
}

func validateEvidenceLine(file *os.File, name string, wanted int) error {
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 64*1024), maxEvidenceLineBytes)
	for current := 1; scanner.Scan(); current++ {
		text := scanner.Text()
		if wanted == 0 {
			if strings.TrimSpace(text) != "" && utf8.ValidString(text) {
				return nil
			}
			continue
		}
		if current == wanted {
			if strings.TrimSpace(text) == "" || !utf8.ValidString(text) {
				return fmt.Errorf("path %q line %d is empty or not UTF-8 text", name, wanted)
			}
			return nil
		}
	}
	if err := scanner.Err(); err != nil {
		return fmt.Errorf("path %q cannot be read as bounded text evidence: %w", name, err)
	}
	if wanted == 0 {
		return fmt.Errorf("path %q contains no non-empty UTF-8 text evidence", name)
	}
	return fmt.Errorf("path %q line %d is outside the file", name, wanted)
}
