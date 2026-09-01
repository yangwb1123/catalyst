package domain

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const deliveryDomainImportPath = "forgeos/forge-core/internal/delivery/domain"
const deliveryReconcilerImportPath = "forgeos/forge-core/internal/reconcile/application"

var dependencyImportPathAllowlist = map[string]bool{
	"bytes": true, "crypto/sha256": true, "encoding/hex": true,
	"encoding/json": true, "errors": true, "fmt": true, "io": true,
	"math": true, "reflect": true, "sort": true, "strconv": true,
	"strings": true, "unicode/utf8": true,
	"forgeos/forge-core/internal/platformcorecontract":                      true,
	"forgeos/forge-core/internal/platformcorecontract/internal/wireprofile": true,
}

func TestPlatformCoreDependencyImportPathsStayAllowlisted(t *testing.T) {
	_, moduleRoot := deliverySourceRoots(t)
	roots := []string{
		filepath.Join(moduleRoot, "internal", "platformcorecontract"),
		filepath.Join(moduleRoot, "internal", "platformcorecontract", "state"),
		filepath.Join(moduleRoot, "internal", "platformcorecontract", "internal", "wireprofile"),
	}
	for _, root := range roots {
		count, err := checkDependencyImports(root)
		if err != nil {
			t.Fatalf("dependency closure %s: %v", quotedSourceDiagnostic(root), err)
		}
		if count == 0 {
			t.Fatalf("dependency closure %s has no production source", quotedSourceDiagnostic(root))
		}
	}
}

func TestDependencyImportPathsRejectCapabilityPackages(t *testing.T) {
	imports := []string{
		"os", "os/exec", "net", "net/http", "database/sql", "time", "math/rand", "crypto/rand",
	}
	for _, imported := range imports {
		source := []byte("package fixture\nimport _ \"" + imported + "\"\n")
		if err := checkSourceImports(
			"fixture.go", source, dependencyImportPathAllowlist,
		); err == nil {
			t.Fatalf("effect import %q entered the dependency closure", imported)
		}
	}
}

func TestDependencyImportAllowlistUsesExactPaths(t *testing.T) {
	for _, imported := range []string{" fmt", "fmt ", "\tfmt", "fmt\u00a0", "\u2003fmt"} {
		source := []byte(fmt.Sprintf("package fixture\nimport _ %q\n", imported))
		if err := checkSourceImports(
			"fixture.go", source, dependencyImportPathAllowlist,
		); err == nil {
			t.Fatalf("normalized import path %q entered the dependency closure", imported)
		}
	}
}

func checkDependencyImports(root string) (int, error) {
	root, before, err := canonicalSourceDirectory(root)
	if err != nil {
		return 0, err
	}
	budget := defaultSourceScanBudget()
	entries, err := readBoundedSourceDirectory(root, 0, budget)
	if err != nil {
		return 0, err
	}
	count := 0
	for _, entry := range entries {
		if entry.Type()&os.ModeSymlink != 0 {
			return 0, fmt.Errorf(
				"symlinked dependency source %s", quotedSourceDiagnostic(entry.Name()),
			)
		}
		if entry.IsDir() {
			continue
		}
		if !strings.HasSuffix(entry.Name(), ".go") {
			return 0, fmt.Errorf(
				"unexpected dependency artifact %s", quotedSourceDiagnostic(entry.Name()),
			)
		}
		source, err := readStableSourceWithBudget(root, entry.Name(), budget)
		if err != nil {
			return 0, err
		}
		if strings.HasSuffix(entry.Name(), "_test.go") {
			continue
		}
		count++
		if err := checkSourceImports(
			filepath.Join(root, entry.Name()), source, dependencyImportPathAllowlist,
		); err != nil {
			return 0, err
		}
	}
	after, err := os.Lstat(root)
	if err != nil || !sameSourceIdentity(before, after) {
		return 0, fmt.Errorf("dependency source root changed during inspection")
	}
	return count, nil
}
