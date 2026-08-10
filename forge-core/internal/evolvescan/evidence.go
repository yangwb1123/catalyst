package evolvescan

import (
	"bufio"
	"bytes"
	"fmt"
	"io"
	"os"
	"path"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	maxEvidenceFileBytes = 1 << 20
	maxEvidenceLineBytes = 1 << 20
)

func validateEvidencePath(root, name string, line int) error {
	return validateEvidencePathObserved(root, name, line, nil)
}

func validateEvidencePathObserved(root, name string, line int, observer evidencePathObserver) error {
	if err := validateEvidencePathText(name); err != nil {
		return err
	}
	opened, err := openEvidencePath(root, name, observer)
	if err != nil {
		return err
	}
	defer opened.close()
	return validateOpenedEvidence(opened, name, line)
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

func validateOpenedEvidence(opened *openedEvidencePath, name string, line int) error {
	info := opened.expected
	if !info.Mode().IsRegular() {
		return fmt.Errorf("path %q changed identity while being validated", name)
	}
	if info.Size() <= 0 {
		return fmt.Errorf("path %q is empty and cannot locate evidence", name)
	}
	if info.Size() > maxEvidenceFileBytes {
		return fmt.Errorf("path %q exceeds the %d-byte evidence read limit", name, maxEvidenceFileBytes)
	}
	if err := validateEvidenceLine(opened.file, name, line, info.Size()); err != nil {
		return err
	}
	return opened.verify()
}

func validateEvidenceLine(file *os.File, name string, wanted int, expectedBytes int64) error {
	data, err := io.ReadAll(io.LimitReader(file, maxEvidenceFileBytes+1))
	if err != nil {
		return fmt.Errorf("path %q cannot be read as bounded text evidence: %w", name, err)
	}
	if int64(len(data)) != expectedBytes || int64(len(data)) > maxEvidenceFileBytes {
		return fmt.Errorf("path %q changed size while being validated", name)
	}
	if !utf8.Valid(data) {
		return fmt.Errorf("path %q is not a complete UTF-8 text file", name)
	}
	scanner := bufio.NewScanner(bytes.NewReader(data))
	scanner.Buffer(make([]byte, 64*1024), maxEvidenceLineBytes+1)
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
