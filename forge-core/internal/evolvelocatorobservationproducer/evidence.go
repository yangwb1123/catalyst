package evolvelocatorobservationproducer

import (
	"bufio"
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"sort"
	"strings"
	"unicode/utf8"

	"forgeos/forge-core/internal/gitworktreesource"
)

func captureEvidenceFiles(
	ctx context.Context,
	root string,
	occurrences []occurrence,
	manifest gitworktreesource.SourceManifest,
) (map[string]fileFact, error) {
	wanted := wantedLines(occurrences)
	tree, err := openRootedTree(root)
	if err != nil {
		return nil, err
	}
	defer func() { _ = tree.handle.Close() }()
	paths := make([]string, 0, len(wanted))
	for path := range wanted {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	facts := make(map[string]fileFact, len(paths))
	for _, path := range paths {
		if err := ctx.Err(); err != nil {
			return nil, fmt.Errorf("capture Evolve locator files: %w", err)
		}
		fact, err := captureEvidenceFile(ctx, tree, path, wanted[path])
		if err != nil {
			return nil, err
		}
		if err := matchSourceManifest(manifest, path, fact); err != nil {
			return nil, err
		}
		facts[path] = fact
	}
	if err := tree.verify(); err != nil {
		return nil, err
	}
	return facts, nil
}

func wantedLines(occurrences []occurrence) map[string]map[int]struct{} {
	result := make(map[string]map[int]struct{})
	for _, item := range occurrences {
		lines := result[item.evidence.Path]
		if lines == nil {
			lines = make(map[int]struct{})
			result[item.evidence.Path] = lines
		}
		lines[item.evidence.Line] = struct{}{}
	}
	return result
}

func captureEvidenceFile(
	ctx context.Context,
	tree *rootedTree,
	path string,
	lines map[int]struct{},
) (fileFact, error) {
	parent, err := openRootedParent(tree, path)
	if err != nil {
		return fileFact{}, err
	}
	defer parent.close()
	before, err := parent.leafRoot.Lstat(parent.leaf)
	if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.Mode().IsRegular() {
		return fileFact{}, fmt.Errorf("locator path %q must be an available non-symlink regular file", path)
	}
	if before.Size() < 1 || before.Size() > maxEvidenceFileBytes {
		return fileFact{}, fmt.Errorf("locator path %q byte size must be 1..%d", path, maxEvidenceFileBytes)
	}
	file, err := parent.leafRoot.Open(parent.leaf)
	if err != nil {
		return fileFact{}, fmt.Errorf("open locator path %q: %w", path, err)
	}
	defer func() { _ = file.Close() }()
	opened, openErr := file.Stat()
	current, currentErr := parent.leafRoot.Lstat(parent.leaf)
	if openErr != nil || currentErr != nil || !stableFile(before, opened) ||
		!stableFile(opened, current) {
		return fileFact{}, fmt.Errorf("locator path %q changed while opening", path)
	}
	data, err := readBoundedFile(ctx, path, file)
	if err != nil {
		return fileFact{}, err
	}
	after, statErr := file.Stat()
	current, currentErr = parent.leafRoot.Lstat(parent.leaf)
	if statErr != nil || currentErr != nil || !stableFile(opened, after) ||
		!stableFile(opened, current) || int64(len(data)) != opened.Size() {
		return fileFact{}, fmt.Errorf("locator path %q changed while reading", path)
	}
	if err := parent.verify(); err != nil {
		return fileFact{}, err
	}
	if err := validateCapturedLines(path, data, lines); err != nil {
		return fileFact{}, err
	}
	return fileFact{bytes: int64(len(data)), sha256: sha256Bytes(data)}, nil
}

func readBoundedFile(ctx context.Context, path string, file *os.File) ([]byte, error) {
	reader := &contextReader{ctx: ctx, reader: io.LimitReader(file, maxEvidenceFileBytes+1)}
	data, err := io.ReadAll(reader)
	if err != nil {
		return nil, fmt.Errorf("read locator path %q: %w", path, err)
	}
	if int64(len(data)) > maxEvidenceFileBytes {
		return nil, fmt.Errorf("locator path %q exceeds %d bytes", path, maxEvidenceFileBytes)
	}
	return data, nil
}

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (reader *contextReader) Read(buffer []byte) (int, error) {
	if err := reader.ctx.Err(); err != nil {
		return 0, err
	}
	return reader.reader.Read(buffer)
}

func validateCapturedLines(path string, data []byte, wanted map[int]struct{}) error {
	if !utf8.Valid(data) {
		return fmt.Errorf("locator path %q is not a complete UTF-8 text file", path)
	}
	found := make(map[int]bool, len(wanted))
	lineZero := false
	scanner := bufio.NewScanner(bytes.NewReader(data))
	scanner.Buffer(make([]byte, 64*1024), int(maxEvidenceFileBytes)+1)
	for line := 1; scanner.Scan(); line++ {
		text := scanner.Text()
		valid := strings.TrimSpace(text) != "" && utf8.ValidString(text)
		if valid {
			lineZero = true
		}
		if _, required := wanted[line]; required && valid {
			found[line] = true
		}
	}
	if err := scanner.Err(); err != nil {
		return fmt.Errorf("locator path %q cannot be scanned as bounded text: %w", path, err)
	}
	if _, required := wanted[0]; required && !lineZero {
		return fmt.Errorf("locator path %q contains no non-empty UTF-8 text evidence", path)
	}
	for line := range wanted {
		if line > 0 && !found[line] {
			return fmt.Errorf("locator path %q line %d is outside, empty, or not UTF-8", path, line)
		}
	}
	return nil
}

func matchSourceManifest(
	manifest gitworktreesource.SourceManifest,
	path string,
	fact fileFact,
) error {
	index := sort.Search(len(manifest.Entries), func(index int) bool {
		return manifest.Entries[index].Path >= path
	})
	if index == len(manifest.Entries) || manifest.Entries[index].Path != path {
		return fmt.Errorf("locator path %q is absent from source manifest", path)
	}
	entry := manifest.Entries[index]
	if entry.Kind != "regular" || entry.ContentSHA256 == nil ||
		entry.Bytes != fact.bytes || *entry.ContentSHA256 != fact.sha256 {
		return fmt.Errorf("locator path %q does not match its regular source entry", path)
	}
	return nil
}
