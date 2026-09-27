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

go 1.26.1

require (
	github.com/yangwb1123/snaplink v0.0.0-20260909212256-7da58fa06b14
	modernc.org/sqlite v1.57.0
)

require (
	github.com/beorn7/perks v1.0.1 // indirect
	github.com/cenkalti/backoff/v5 v5.0.3 // indirect
	github.com/cespare/xxhash/v2 v2.3.0 // indirect
	github.com/dustin/go-humanize v1.0.1 // indirect
	github.com/felixge/httpsnoop v1.0.4 // indirect
	github.com/go-jose/go-jose/v4 v4.1.4 // indirect
	github.com/go-logr/logr v1.4.4 // indirect
	github.com/go-logr/stdr v1.2.2 // indirect
	github.com/goccy/go-yaml v1.19.2 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/grpc-ecosystem/grpc-gateway/v2 v2.29.0 // indirect
	github.com/mattn/go-isatty v0.0.24 // indirect
	github.com/munnerz/goautoneg v0.0.0-20191010083416-a7dc8b61c822 // indirect
	github.com/ncruces/go-strftime v1.0.0 // indirect
	github.com/prometheus/client_golang v1.24.0 // indirect
	github.com/prometheus/client_model v0.6.2 // indirect
	github.com/prometheus/common v0.70.0 // indirect
	github.com/prometheus/procfs v0.21.1 // indirect
	github.com/remyoudompheng/bigfft v0.0.0-20230129092748-24d4a6f8daec // indirect
	github.com/tetratelabs/wazero v1.12.0 // indirect
	go.opentelemetry.io/auto/sdk v1.2.1 // indirect
	go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp v0.69.0 // indirect
	go.opentelemetry.io/otel v1.44.0 // indirect
	go.opentelemetry.io/otel/exporters/otlp/otlptrace v1.44.0 // indirect
	go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracegrpc v1.44.0 // indirect
	go.opentelemetry.io/otel/metric v1.44.0 // indirect
	go.opentelemetry.io/otel/sdk v1.44.0 // indirect
	go.opentelemetry.io/otel/trace v1.44.0 // indirect
	go.opentelemetry.io/proto/otlp v1.10.0 // indirect
	golang.org/x/crypto v0.53.0 // indirect
	golang.org/x/net v0.56.0 // indirect
	golang.org/x/sync v0.22.0 // indirect
	golang.org/x/sys v0.47.0 // indirect
	golang.org/x/text v0.40.0 // indirect
	golang.org/x/time v0.15.0 // indirect
	google.golang.org/genproto/googleapis/api v0.0.0-20260526163538-3dc84a4a5aaa // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20260526163538-3dc84a4a5aaa // indirect
	google.golang.org/grpc v1.82.1 // indirect
	google.golang.org/protobuf v1.36.11 // indirect
	modernc.org/libc v1.74.4 // indirect
	modernc.org/mathutil v1.7.1 // indirect
	modernc.org/memory v1.11.0 // indirect
)

replace github.com/yangwb1123/snaplink => /home/u1/workspace/demo/snaplink
`

const expectedGoSumSHA256 = "915094002e7559dcba2590fd5b13eec18d5fd74d39b654dc00488892c9078e33"

// CheckModuleDependencyPolicy enforces the exact reviewed Snaplink-auth and
// SQLite module closure and confines product imports to their approved boundaries.
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
		if isApprovedExternalImport(relative, importPath) {
			return nil
		}
		return fmt.Errorf("%s imports unapproved external module %s", relative, importPath)
	}
	return nil
}

func isApprovedExternalImport(relative, importPath string) bool {
	if strings.HasPrefix(filepath.ToSlash(relative), "internal/appserver/") &&
		strings.HasSuffix(relative, "_test.go") &&
		strings.HasPrefix(importPath, "github.com/yangwb1123/snaplink/") {
		return true
	}
	if filepath.ToSlash(relative) != "internal/authn/authn.go" {
		return false
	}
	switch importPath {
	case "github.com/yangwb1123/snaplink/interfaces/ssoclient/remote",
		"github.com/yangwb1123/snaplink/interfaces/ssoclient/rs":
		return true
	default:
		return false
	}
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
