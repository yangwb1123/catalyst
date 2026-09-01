package domain

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

var productionImportAllowlist = map[string]bool{
	"errors": true, "sort": true, "strings": true,
	"unicode": true, "unicode/utf8": true,
	"forgeos/forge-core/internal/platformcorecontract":       true,
	"forgeos/forge-core/internal/platformcorecontract/state": true,
}

func TestProductionImportAllowlist(t *testing.T) {
	root, _ := deliverySourceRoots(t)
	count, err := checkProductionImports(root)
	if err != nil {
		t.Fatal(err)
	}
	if count == 0 {
		t.Fatal("production import scan found no Go source")
	}
}

func checkProductionImports(root string) (int, error) {
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
		production, err := checkProductionEntry(root, entry, budget)
		if err != nil {
			return 0, err
		}
		if production {
			count++
		}
	}
	after, err := os.Lstat(root)
	if err != nil || !sameSourceIdentity(before, after) {
		return 0, fmt.Errorf("production source root changed during inspection")
	}
	return count, nil
}

func checkProductionEntry(
	root string, entry os.DirEntry, budget *sourceScanBudget,
) (bool, error) {
	if entry.Type()&os.ModeSymlink != 0 {
		return false, fmt.Errorf(
			"symlinked production artifact %s is forbidden", quotedSourceDiagnostic(entry.Name()),
		)
	}
	if entry.IsDir() {
		if entry.Name() == "testdata" {
			return false, nil
		}
		return false, fmt.Errorf(
			"unexpected production subdirectory %s", quotedSourceDiagnostic(entry.Name()),
		)
	}
	if !strings.HasSuffix(entry.Name(), ".go") {
		return false, fmt.Errorf(
			"unexpected non-Go production artifact %s", quotedSourceDiagnostic(entry.Name()),
		)
	}
	source, err := readStableSourceWithBudget(root, entry.Name(), budget)
	if err != nil {
		return false, err
	}
	if strings.HasSuffix(entry.Name(), "_test.go") {
		return false, nil
	}
	path := filepath.Join(root, entry.Name())
	return true, checkSourceImports(path, source, productionImportAllowlist)
}

func TestProductionBoundaryRejectsCompiledNonGoArtifacts(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(root, "domain.go"), []byte("package domain\n"), 0o600,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "effects.s"), []byte("TEXT effect(SB),$0\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := checkProductionImports(root); err == nil {
		t.Fatal("compiled non-Go artifact bypassed the production boundary")
	}
}

func TestProductionBoundaryRejectsSymlinkedGoSources(t *testing.T) {
	root := t.TempDir()
	targetRoot := t.TempDir()
	target := filepath.Join(targetRoot, "outside.go")
	if err := os.WriteFile(target, []byte("package domain\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "domain.go")
	if err := os.WriteFile(source, []byte("package domain\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := checkProductionImports(root); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(source); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, source); err != nil {
		t.Skipf("symlink creation is unavailable: %v", err)
	}
	if _, err := checkProductionImports(root); err == nil {
		t.Fatal("symlinked Go source bypassed the production boundary")
	}
}

func TestProductionBoundaryRejectsSymlinkedPackageRoot(t *testing.T) {
	target := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(target, "domain.go"), []byte("package domain\n"), 0o600,
	); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(t.TempDir(), "domain")
	if err := os.Symlink(target, alias); err != nil {
		t.Skipf("symlink creation is unavailable: %v", err)
	}
	if _, err := checkProductionImports(alias); err == nil {
		t.Fatal("symlinked package root bypassed the production boundary")
	}
}

func TestProductionBoundaryRejectsHardLinkedGoSources(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(root, "domain.go"), []byte("package domain\n"), 0o600,
	); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(t.TempDir(), "outside.go")
	if err := os.WriteFile(outside, []byte("package domain\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(outside, filepath.Join(root, "linked.go")); err != nil {
		t.Skipf("hard-link creation is unavailable: %v", err)
	}
	if _, err := checkProductionImports(root); err == nil {
		t.Fatal("hard-linked Go source bypassed the production boundary")
	}
}

func deliverySourceRoots(t *testing.T) (string, string) {
	t.Helper()
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("cannot resolve delivery domain source directory")
	}
	packageRoot := filepath.Dir(source)
	moduleRoot := filepath.Clean(filepath.Join(packageRoot, "..", "..", ".."))
	if err := requireContainedSourceRoot(moduleRoot, packageRoot); err != nil {
		t.Fatal(err)
	}
	return packageRoot, moduleRoot
}
