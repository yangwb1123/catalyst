package doctor

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

const expectedGoMod = `module forgeos/forge-core

go 1.26

require modernc.org/sqlite v1.57.0

require (
	github.com/dustin/go-humanize v1.0.1 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/mattn/go-isatty v0.0.24 // indirect
	github.com/ncruces/go-strftime v1.0.0 // indirect
	github.com/remyoudompheng/bigfft v0.0.0-20230129092748-24d4a6f8daec // indirect
	golang.org/x/sys v0.47.0 // indirect
	modernc.org/libc v1.74.4 // indirect
	modernc.org/mathutil v1.7.1 // indirect
	modernc.org/memory v1.11.0 // indirect
)
`

const expectedGoSumSHA256 = "9146e7666d70f22e7739e8735d90275a271753977bd21337caf82345fd8f3cd0"

// CheckModuleDependencyPolicy enforces the exact reviewed SQLite module
// closure and confines its only direct product import to controlstore.
func CheckModuleDependencyPolicy(forgeDir string) error {
	goMod, err := os.ReadFile(filepath.Join(forgeDir, "go.mod"))
	if err != nil {
		return fmt.Errorf("read go.mod: %w", err)
	}
	goSum, err := os.ReadFile(filepath.Join(forgeDir, "go.sum"))
	if err != nil {
		return fmt.Errorf("read go.sum: %w", err)
	}
	if err := CheckModuleManifests(goMod, goSum); err != nil {
		return err
	}
	return checkProductImports(forgeDir)
}

// CheckModuleManifests validates exact dependency declarations and checksums.
func CheckModuleManifests(goMod, goSum []byte) error {
	if !bytes.Equal(goMod, []byte(expectedGoMod)) {
		return fmt.Errorf("go.mod differs from the reviewed control-store dependency policy")
	}
	digest := fmt.Sprintf("%x", sha256.Sum256(goSum))
	if digest != expectedGoSumSHA256 {
		return fmt.Errorf("go.sum digest %s is outside the reviewed closure", digest)
	}
	return nil
}

func checkProductImports(forgeDir string) error {
	for _, directory := range []string{"cmd", "internal"} {
		root := filepath.Join(forgeDir, directory)
		if err := filepath.WalkDir(root, importWalk(forgeDir)); err != nil {
			return fmt.Errorf("scan %s imports: %w", directory, err)
		}
	}
	return nil
}

func importWalk(forgeDir string) fs.WalkDirFunc {
	return func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") {
			return nil
		}
		return checkGoFileImports(forgeDir, path)
	}
}

func checkGoFileImports(forgeDir, path string) error {
	parsed, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
	if err != nil {
		return fmt.Errorf("parse %s: %w", path, err)
	}
	relative, err := filepath.Rel(forgeDir, path)
	if err != nil {
		return err
	}
	for _, spec := range parsed.Imports {
		if err := checkImport(relative, spec.Name, spec.Path.Value); err != nil {
			return err
		}
	}
	return nil
}

func checkImport(relative string, name *ast.Ident, quotedPath string) error {
	importPath, err := strconv.Unquote(quotedPath)
	if err != nil {
		return fmt.Errorf("%s has invalid import %s", relative, quotedPath)
	}
	if importPath == "modernc.org/sqlite" {
		if filepath.ToSlash(relative) == "internal/controlstore/open_linux.go" && blankImport(name) {
			return nil
		}
		return fmt.Errorf("%s imports SQLite outside its sole allowed boundary", relative)
	}
	if importPath == "C" {
		return fmt.Errorf("%s introduces an unapproved CGo boundary", relative)
	}
	if isExternalImport(importPath) {
		return fmt.Errorf("%s imports unapproved external module %s", relative, importPath)
	}
	return nil
}

func blankImport(name *ast.Ident) bool {
	return name != nil && name.Name == "_"
}

func isExternalImport(importPath string) bool {
	if strings.HasPrefix(importPath, "forgeos/forge-core/") {
		return false
	}
	first, _, _ := strings.Cut(importPath, "/")
	return strings.Contains(first, ".")
}
