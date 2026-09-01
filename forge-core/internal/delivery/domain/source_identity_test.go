package domain

import (
	"fmt"
	"go/parser"
	"go/token"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strconv"
)

const maxInspectedSourceBytes = 1 << 20
const sourceScanBatchEntries = 128
const maxSourceScanEntries = 16384
const maxSourceScanFiles = 12288
const maxSourceScanDepth = 64
const maxSourceScanBytes = int64(256 << 20)
const maxSourceDiagnosticBytes = 1 << 10
const maxSourceDiagnosticValueBytes = 192

type sourceScanBudget struct {
	maxEntries int
	maxFiles   int
	maxDepth   int
	maxBytes   int64
	entries    int
	files      int
	bytes      int64
}

func defaultSourceScanBudget() *sourceScanBudget {
	return &sourceScanBudget{
		maxEntries: maxSourceScanEntries, maxFiles: maxSourceScanFiles,
		maxDepth: maxSourceScanDepth, maxBytes: maxSourceScanBytes,
	}
}

func (budget *sourceScanBudget) addEntry(depth int, directory bool) error {
	if depth < 1 || depth > budget.maxDepth {
		return fmt.Errorf("source scan depth exceeds its bound")
	}
	if budget.entries >= budget.maxEntries {
		return fmt.Errorf("source scan entry count exceeds its bound")
	}
	budget.entries++
	if !directory {
		if budget.files >= budget.maxFiles {
			return fmt.Errorf("source scan file count exceeds its bound")
		}
		budget.files++
	}
	return nil
}

func (budget *sourceScanBudget) reserveSourceBytes(count int64) error {
	if count < 0 || budget.bytes > budget.maxBytes || count > budget.maxBytes-budget.bytes {
		return fmt.Errorf("source scan aggregate bytes exceed their bound")
	}
	budget.bytes += count
	return nil
}

func readBoundedSourceDirectory(
	path string, depth int, budget *sourceScanBudget,
) ([]os.DirEntry, error) {
	before, err := os.Lstat(path)
	if err != nil {
		return nil, sourceOperationError("inspect source directory", path, err)
	}
	if before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, fmt.Errorf("source scan directory is invalid: %s", quotedSourceDiagnostic(path))
	}
	directory, err := os.Open(path)
	if err != nil {
		return nil, sourceOperationError("open source directory", path, err)
	}
	defer directory.Close()
	opened, err := directory.Stat()
	if err != nil {
		return nil, sourceOperationError("stat opened source directory", path, err)
	}
	if !sameSourceIdentity(before, opened) {
		return nil, fmt.Errorf(
			"source scan directory changed before open: %s", quotedSourceDiagnostic(path),
		)
	}
	entries, err := readSourceDirectoryBatches(path, directory, depth, budget)
	if err != nil {
		return nil, err
	}
	if err := verifyStableSourceDirectory(path, before, directory); err != nil {
		return nil, err
	}
	sort.Slice(entries, func(left, right int) bool {
		return entries[left].Name() < entries[right].Name()
	})
	return entries, nil
}

func readSourceDirectoryBatches(
	path string, directory *os.File, depth int, budget *sourceScanBudget,
) ([]os.DirEntry, error) {
	entries := make([]os.DirEntry, 0, sourceScanBatchEntries)
	for {
		batch, err := directory.ReadDir(sourceScanBatchEntries)
		for _, entry := range batch {
			if budgetErr := budget.addEntry(depth+1, entry.IsDir()); budgetErr != nil {
				return nil, budgetErr
			}
			entries = append(entries, entry)
		}
		if err == io.EOF {
			return entries, nil
		}
		if err != nil {
			return nil, sourceOperationError("read source directory", path, err)
		}
	}
}

func verifyStableSourceDirectory(path string, before os.FileInfo, directory *os.File) error {
	openedAfter, err := directory.Stat()
	if err != nil {
		return sourceOperationError("restat opened source directory", path, err)
	}
	pathAfter, err := os.Lstat(path)
	if err != nil || !sameSourceVersion(before, openedAfter) ||
		!sameSourceVersion(before, pathAfter) {
		return fmt.Errorf(
			"source scan directory changed during inspection: %s", quotedSourceDiagnostic(path),
		)
	}
	return nil
}

func walkBoundedSourceTree(
	root string, budget *sourceScanBudget,
	visit func(string, os.DirEntry) error,
) error {
	return walkBoundedSourceDirectory(root, 0, budget, visit)
}

func walkBoundedSourceDirectory(
	root string, depth int, budget *sourceScanBudget,
	visit func(string, os.DirEntry) error,
) error {
	entries, err := readBoundedSourceDirectory(root, depth, budget)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		path := filepath.Join(root, entry.Name())
		if err := visit(path, entry); err != nil {
			return err
		}
		if entry.IsDir() {
			if err := walkBoundedSourceDirectory(path, depth+1, budget, visit); err != nil {
				return err
			}
		}
	}
	return nil
}

func canonicalSourceDirectory(root string) (string, os.FileInfo, error) {
	absolute, err := filepath.Abs(root)
	if err != nil {
		return "", nil, sourceOperationError("resolve absolute source root", root, err)
	}
	absolute = filepath.Clean(absolute)
	info, err := os.Lstat(absolute)
	if err != nil {
		return "", nil, sourceOperationError("inspect source root", absolute, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return "", nil, fmt.Errorf("source root must be a non-symlink directory")
	}
	resolved, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", nil, sourceOperationError("resolve source root aliases", absolute, err)
	}
	if filepath.Clean(resolved) != absolute {
		return "", nil, fmt.Errorf("source root traverses a symlink")
	}
	return absolute, info, nil
}

func requireContainedSourceRoot(moduleRoot, packageRoot string) error {
	moduleRoot, _, err := canonicalSourceDirectory(moduleRoot)
	if err != nil {
		return err
	}
	packageRoot, _, err = canonicalSourceDirectory(packageRoot)
	if err != nil {
		return err
	}
	relative, err := filepath.Rel(moduleRoot, packageRoot)
	if err != nil {
		return fmt.Errorf(
			"relate source roots %s and %s: %s", quotedSourceDiagnostic(moduleRoot),
			quotedSourceDiagnostic(packageRoot), quotedSourceDiagnostic(err.Error()),
		)
	}
	if filepath.ToSlash(relative) != "internal/delivery/domain" {
		return fmt.Errorf("delivery source root escapes its exact module location")
	}
	return nil
}

func readStableSource(root, name string) ([]byte, error) {
	return readStableSourceWithBudget(root, name, nil)
}

func readStableSourceWithBudget(
	root, name string, budget *sourceScanBudget,
) ([]byte, error) {
	path := filepath.Join(root, name)
	before, err := os.Lstat(path)
	if err != nil {
		return nil, sourceOperationError("inspect source file", path, err)
	}
	if err := requireSingleLinkRegular(path, before, name); err != nil {
		return nil, err
	}
	if before.Size() < 0 || before.Size() > int64(maxInspectedSourceBytes) {
		return nil, fmt.Errorf("source read is invalid or over-bound: %s", quotedSourceDiagnostic(name))
	}
	if budget != nil {
		if err := budget.reserveSourceBytes(before.Size()); err != nil {
			return nil, err
		}
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, sourceOperationError("open source file", path, err)
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil {
		return nil, sourceOperationError("stat opened source file", path, err)
	}
	if !sameSourceIdentity(before, opened) {
		return nil, fmt.Errorf("source identity changed before open: %s", quotedSourceDiagnostic(name))
	}
	source, err := io.ReadAll(io.LimitReader(file, maxInspectedSourceBytes+1))
	if err != nil {
		return nil, sourceOperationError("read source file", path, err)
	}
	if len(source) > maxInspectedSourceBytes || int64(len(source)) != before.Size() {
		return nil, fmt.Errorf("source read is invalid or over-bound: %s", quotedSourceDiagnostic(name))
	}
	return verifyStableSource(path, name, before, file, source)
}

func verifyStableSource(
	path, name string, before os.FileInfo, file *os.File, source []byte,
) ([]byte, error) {
	openedAfter, err := file.Stat()
	if err != nil {
		return nil, sourceOperationError("restat opened source file", path, err)
	}
	pathAfter, err := os.Lstat(path)
	if err != nil {
		return nil, sourceOperationError("reinspect source file", path, err)
	}
	if !sameSourceVersion(before, openedAfter) || !sameSourceVersion(before, pathAfter) {
		return nil, fmt.Errorf("source changed during inspection: %s", quotedSourceDiagnostic(name))
	}
	if err := requireSingleLinkRegular(path, pathAfter, name); err != nil {
		return nil, err
	}
	return source, nil
}

func requireSingleLinkRegular(path string, info os.FileInfo, name string) error {
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return fmt.Errorf(
			"source must be a regular non-symlink file: %s", quotedSourceDiagnostic(name),
		)
	}
	links, ok := sourceLinkCount(path, info)
	if !ok || links != 1 {
		return fmt.Errorf("source must have one inspectable link: %s", quotedSourceDiagnostic(name))
	}
	return nil
}

func sameSourceIdentity(left, right os.FileInfo) bool {
	return left != nil && right != nil && os.SameFile(left, right) && left.Mode() == right.Mode()
}

func sameSourceVersion(left, right os.FileInfo) bool {
	return sameSourceIdentity(left, right) && left.Size() == right.Size() &&
		left.ModTime().Equal(right.ModTime())
}

func checkSourceImports(path string, source []byte, allowlist map[string]bool) error {
	imports, err := parseSourceImports(path, source)
	if err != nil {
		return err
	}
	for _, name := range imports {
		if !allowlist[name] {
			return fmt.Errorf(
				"production import %s in %s is forbidden",
				quotedSourceDiagnostic(name), quotedSourceDiagnostic(path),
			)
		}
	}
	return nil
}

func parseSourceImports(path string, source []byte) ([]string, error) {
	parsed, err := parser.ParseFile(token.NewFileSet(), "inspected-source.go", source, parser.ImportsOnly)
	if err != nil {
		return nil, fmt.Errorf(
			"parse source %s: %s", quotedSourceDiagnostic(path), quotedSourceDiagnostic(err.Error()),
		)
	}
	result := make([]string, 0, len(parsed.Imports))
	for _, imported := range parsed.Imports {
		name, err := strconv.Unquote(imported.Path.Value)
		if err != nil {
			return nil, fmt.Errorf(
				"decode source import %s: %s", quotedSourceDiagnostic(imported.Path.Value),
				quotedSourceDiagnostic(err.Error()),
			)
		}
		result = append(result, name)
	}
	return result, nil
}

func quotedSourceDiagnostic(value string) string {
	truncated := len(value) > maxSourceDiagnosticValueBytes
	if truncated {
		value = value[:maxSourceDiagnosticValueBytes]
	}
	result := strconv.QuoteToASCII(value)
	if truncated {
		result += " …[value truncated]"
	}
	return result
}

func sourceOperationError(operation, path string, err error) error {
	return fmt.Errorf(
		"%s %s failed: %s", operation, quotedSourceDiagnostic(path),
		quotedSourceDiagnostic(err.Error()),
	)
}
