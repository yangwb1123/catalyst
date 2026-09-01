package domain

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	"forgeos/forge-core/internal/execbound"
)

const maxGoListOutputBytes = 16 << 20
const maxGoListStreamBytes = maxGoListOutputBytes / 2
const maxGoListDiagnosticBytes = 4 << 10
const maxGoListDiagnosticRawBytes = 960
const maxGoListMetadataTextBytes = 4 << 10
const maxConsumerSourcePaths = 1024
const maxConsumerDiagnosticPaths = 3
const maxConsumerDiagnosticPathBytes = 256
const maxConsumerDiagnosticBytes = 4 << 10

type listedProductionPackage struct {
	ImportPath string
	Dir        string
	Imports    []string
}

func TestDeliveryDomainHasOnlyTheReviewedReconcilerConsumer(t *testing.T) {
	_, moduleRoot := deliverySourceRoots(t)
	lexical, err := scanModuleConsumerSources(moduleRoot)
	if err != nil {
		t.Fatal(err)
	}
	reconcilerDir := filepath.Join(moduleRoot, "internal", "reconcile", "application")
	if len(lexical) == 0 {
		t.Fatal("reviewed Reconciler source consumer is missing")
	}
	for _, path := range lexical {
		if filepath.Dir(path) != reconcilerDir {
			t.Fatalf("mutable Delivery drafts gained another source consumer: %s",
				boundedConsumerDiagnostic(lexical))
		}
	}
	reconcilerConsumers, err := scanModuleConsumerSourcesFor(
		moduleRoot, defaultSourceScanBudget(), deliveryReconcilerImportPath,
	)
	if err != nil || len(reconcilerConsumers) != 0 {
		t.Fatalf("passive Reconciler gained source consumers: %s, error=%v",
			boundedConsumerDiagnostic(reconcilerConsumers), err)
	}
	t.Setenv("GOPROXY", "https://hostile.invalid")
	t.Setenv("GOVCS", "*:all")
	t.Setenv("GOENV", filepath.Join(t.TempDir(), "hostile-goenv"))
	isolatedRoot := t.TempDir()
	for _, target := range []string{"linux", "darwin", "windows"} {
		packages, err := listProductionPackages(moduleRoot, target, isolatedRoot)
		if err != nil {
			t.Fatalf("%s production package metadata: %v", target, err)
		}
		assertDomainMetadataImports(t, target, packages)
		consumers := productionConsumers(packages)
		if strings.Join(consumers, ",") != deliveryReconcilerImportPath {
			t.Fatalf("%s mutable Delivery consumers: %s", target,
				boundedConsumerDiagnostic(consumers))
		}
		if consumers := productionConsumersOf(packages, deliveryReconcilerImportPath); len(consumers) != 0 {
			t.Fatalf("%s passive Reconciler consumers: %s", target,
				boundedConsumerDiagnostic(consumers))
		}
	}
}

func assertDomainMetadataImports(
	t *testing.T, target string, packages []listedProductionPackage,
) {
	t.Helper()
	for _, value := range packages {
		if value.ImportPath != deliveryDomainImportPath {
			continue
		}
		imports := stringSet(value.Imports...)
		for _, required := range []string{
			"sort", "forgeos/forge-core/internal/platformcorecontract",
			"forgeos/forge-core/internal/platformcorecontract/state",
		} {
			if !imports[required] {
				t.Fatalf("%s Delivery metadata omitted direct import %q", target, required)
			}
		}
		return
	}
	t.Fatalf("%s package metadata omitted Delivery Domain", target)
}

func listProductionPackages(
	moduleRoot, goos, isolatedRoot string,
) ([]listedProductionPackage, error) {
	environment := goListEnvironment(
		goos, filepath.Join(isolatedRoot, goos, "home"),
		filepath.Join(isolatedRoot, goos, "cache"),
	)
	return runProductionPackageList(moduleRoot, environment)
}

func runProductionPackageList(
	moduleRoot string, environment []string,
) ([]listedProductionPackage, error) {
	goBinary := filepath.Join(runtime.GOROOT(), "bin", "go")
	result := execbound.Run(
		context.Background(), []string{"go", "list", "-mod=readonly", "-e", "-json", "./..."},
		execbound.Options{Timeout: 30 * time.Second, MaxOutputBytes: maxGoListStreamBytes},
		execbound.CaptureSplit, execbound.Spec{
			Dir: moduleRoot, Env: environment, ExecutablePath: goBinary,
		},
	)
	if result.TimedOut() {
		return nil, fmt.Errorf("go list exceeded its 30-second bound")
	}
	if err := validateGoListOutput(result); err != nil {
		return nil, err
	}
	if result.Err != nil {
		return nil, fmt.Errorf(
			"go list failed: cause=%s stderr=%s",
			boundedGoListDiagnostic([]byte(result.Err.Error())), boundedGoListDiagnostic(result.Stderr),
		)
	}
	return decodeProductionPackages(result.Stdout)
}

func validateGoListOutput(result execbound.Result) error {
	if result.DrainIncomplete || result.CountOverflow || result.Total > maxGoListOutputBytes ||
		result.Total > int64(result.Retained) {
		return fmt.Errorf("go list aggregate output exceeded its bound")
	}
	return nil
}

func boundedGoListDiagnostic(source []byte) string {
	truncated := len(source) > maxGoListDiagnosticRawBytes
	if truncated {
		source = source[:maxGoListDiagnosticRawBytes]
	}
	result := strconv.QuoteToASCII(string(source))
	if truncated {
		result += " …[go list stderr diagnostic truncated]"
	}
	return result
}

func boundedConsumerDiagnostic(paths []string) string {
	examples := make([]string, 0, maxConsumerDiagnosticPaths)
	for _, path := range paths {
		if len(examples) == maxConsumerDiagnosticPaths {
			break
		}
		if len(path) > maxConsumerDiagnosticPathBytes {
			path = path[:maxConsumerDiagnosticPathBytes-3] + "..."
		}
		examples = append(examples, strconv.QuoteToASCII(path))
	}
	return fmt.Sprintf("count=%d examples=[%s]", len(paths), strings.Join(examples, ","))
}

func goListEnvironment(goos, home, cache string) []string {
	goPath := filepath.Join(home, "go")
	return []string{
		"HOME=" + home, "TMPDIR=" + os.TempDir(), "PATH=", "LANG=C", "LC_ALL=C",
		"GOROOT=" + runtime.GOROOT(), "GOPATH=" + goPath,
		"GOMODCACHE=" + filepath.Join(goPath, "pkg", "mod"),
		"GOCACHE=" + filepath.Join(cache, "go-build"),
		"GOENV=off", "GOWORK=off", "GOTOOLCHAIN=local", "GOFLAGS=", "GO111MODULE=on",
		"GOOS=" + goos, "GOARCH=amd64", "GOAMD64=v1", "CGO_ENABLED=0",
		"GOPROXY=off", "GONOPROXY=none", "GOPRIVATE=", "GONOSUMDB=none",
		"GOSUMDB=off", "GOVCS=*:off", "GOINSECURE=none", "GOTELEMETRY=off",
		"HTTP_PROXY=", "HTTPS_PROXY=", "ALL_PROXY=", "NO_PROXY=*", "SSH_AUTH_SOCK=",
	}
}

func TestGoListEnvironmentClosesNetworkAndHostOverrides(t *testing.T) {
	t.Setenv("GOPROXY", "https://hostile.invalid")
	t.Setenv("GOVCS", "*:all")
	t.Setenv("GOENV", filepath.Join(t.TempDir(), "hostile-goenv"))
	values := environmentValues(goListEnvironment("linux", "/closed-home", "/closed-cache"))
	want := map[string]string{
		"GOENV": "off", "GOPROXY": "off", "GONOPROXY": "none",
		"GOPRIVATE": "", "GOSUMDB": "off", "GOVCS": "*:off", "PATH": "",
	}
	for _, key := range []string{"GOENV", "GOPROXY", "GONOPROXY", "GOPRIVATE", "GOSUMDB", "GOVCS", "PATH"} {
		if values[key] != want[key] {
			t.Fatalf("closed environment %s = %q, want %q", key, values[key], want[key])
		}
	}
}

func environmentValues(environment []string) map[string]string {
	result := make(map[string]string, len(environment))
	for _, declaration := range environment {
		key, value, ok := strings.Cut(declaration, "=")
		if ok {
			result[key] = value
		}
	}
	return result
}

func decodeProductionPackages(source []byte) ([]listedProductionPackage, error) {
	decoder := json.NewDecoder(bytes.NewReader(source))
	result := make([]listedProductionPackage, 0, 128)
	for {
		var value listedProductionPackage
		err := decoder.Decode(&value)
		if err == io.EOF {
			if len(result) == 0 {
				return nil, fmt.Errorf("go list returned empty package metadata")
			}
			return result, nil
		}
		if err != nil {
			return nil, err
		}
		if !validListedProductionPackage(value) || len(result) >= 4096 {
			return nil, fmt.Errorf("go list package metadata is malformed or over-bound")
		}
		result = append(result, value)
	}
}

func validListedProductionPackage(value listedProductionPackage) bool {
	if !validGoListMetadataText(value.ImportPath, true) ||
		!validGoListMetadataText(value.Dir, false) || len(value.Imports) > 4096 {
		return false
	}
	for _, imported := range value.Imports {
		if !validGoListMetadataText(imported, true) {
			return false
		}
	}
	return true
}

func validGoListMetadataText(value string, required bool) bool {
	return (!required || value != "") && len(value) <= maxGoListMetadataTextBytes &&
		utf8.ValidString(value) && !strings.ContainsAny(value, "\x00\r\n")
}

func TestGoListFailureDiagnosticIsBounded(t *testing.T) {
	value := boundedGoListDiagnostic([]byte(
		"line\nbidi\u202e" + strings.Repeat("\x01", maxGoListDiagnosticBytes+1024),
	))
	if len(value) > maxGoListDiagnosticBytes || strings.Contains(value, "\n") ||
		strings.Contains(value, "\u202e") || !strings.Contains(value, `\n`) ||
		!strings.Contains(value, `\u202e`) || !strings.Contains(value, "diagnostic truncated") {
		t.Fatalf("bounded diagnostic length/content = %d, %q", len(value), value[len(value)-80:])
	}
}

func TestGoListMetadataRejectsUnboundedOrFramedNames(t *testing.T) {
	for _, value := range []listedProductionPackage{
		{ImportPath: strings.Repeat("x", maxGoListMetadataTextBytes+1)},
		{ImportPath: "forgeos/unsafe\nname"},
		{ImportPath: "forgeos/safe", Imports: []string{"forgeos/unsafe\x00name"}},
	} {
		if validListedProductionPackage(value) {
			t.Fatalf("unsafe package metadata was accepted: %s", boundedConsumerDiagnostic([]string{value.ImportPath}))
		}
	}
}

func TestGoListAggregateOutputBound(t *testing.T) {
	for _, test := range []struct {
		name   string
		result execbound.Result
		valid  bool
	}{
		{"exact aggregate limit", execbound.Result{
			Total: maxGoListOutputBytes, Retained: maxGoListOutputBytes,
		}, true},
		{"one byte over aggregate limit", execbound.Result{
			Total: maxGoListOutputBytes + 1, Retained: maxGoListOutputBytes + 1,
		}, false},
		{"one stream truncated", execbound.Result{Total: 100, Retained: 99}, false},
		{"byte count overflow", execbound.Result{
			Total: maxGoListOutputBytes, Retained: maxGoListOutputBytes, CountOverflow: true,
		}, false},
		{"incomplete drain", execbound.Result{
			Total: 1, Retained: 1, DrainIncomplete: true,
		}, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := validateGoListOutput(test.result)
			if (err == nil) != test.valid {
				t.Fatalf("validateGoListOutput() = %v, valid = %v", err, test.valid)
			}
		})
	}
}

func productionConsumers(packages []listedProductionPackage) []string {
	return productionConsumersOf(packages, deliveryDomainImportPath)
}

func productionConsumersOf(
	packages []listedProductionPackage, targetImportPath string,
) []string {
	seen := map[string]bool{}
	for _, value := range packages {
		if value.ImportPath == targetImportPath {
			continue
		}
		for _, imported := range value.Imports {
			if imported == targetImportPath {
				seen[value.ImportPath] = true
			}
		}
	}
	result := make([]string, 0, len(seen))
	for consumer := range seen {
		result = append(result, consumer)
	}
	sort.Strings(result)
	return result
}

func scanModuleConsumerSources(moduleRoot string) ([]string, error) {
	return scanModuleConsumerSourcesWithBudget(moduleRoot, defaultSourceScanBudget())
}

func scanModuleConsumerSourcesWithBudget(
	moduleRoot string, budget *sourceScanBudget,
) ([]string, error) {
	return scanModuleConsumerSourcesFor(
		moduleRoot, budget, deliveryDomainImportPath,
	)
}

func scanModuleConsumerSourcesFor(
	moduleRoot string, budget *sourceScanBudget, targetImportPath string,
) ([]string, error) {
	moduleRoot, _, err := canonicalSourceDirectory(moduleRoot)
	if err != nil {
		return nil, err
	}
	result := []string{}
	err = walkBoundedSourceTree(moduleRoot, budget, func(path string, entry os.DirEntry) error {
		if path != moduleRoot && entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("module source contains a symlink: %s", quotedSourceDiagnostic(path))
		}
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") ||
			strings.HasSuffix(entry.Name(), "_test.go") {
			return nil
		}
		source, err := readStableSourceWithBudget(filepath.Dir(path), entry.Name(), budget)
		if err != nil {
			return err
		}
		imports, err := parseSourceImports(path, source)
		if err != nil {
			return err
		}
		for _, imported := range imports {
			if imported == targetImportPath {
				if len(result) >= maxConsumerSourcePaths {
					return fmt.Errorf("consumer source path count exceeds its bound")
				}
				result = append(result, path)
				break
			}
		}
		return nil
	})
	sort.Strings(result)
	return result, err
}

func TestConsumerGraphFindsDirectImportersIncludingReachableTestdata(t *testing.T) {
	packages := []listedProductionPackage{
		{ImportPath: "forgeos/forge-core/root", Imports: []string{
			"forgeos/forge-core/internal/bridge", "forgeos/forge-core/internal/testdata/wrapper",
		}},
		{ImportPath: "forgeos/forge-core/internal/bridge", Imports: []string{deliveryDomainImportPath}},
		{ImportPath: "forgeos/forge-core/internal/testdata/wrapper", Imports: []string{deliveryDomainImportPath}},
		{ImportPath: deliveryDomainImportPath},
	}
	want := "forgeos/forge-core/internal/bridge,forgeos/forge-core/internal/testdata/wrapper"
	if got := strings.Join(productionConsumers(packages), ","); got != want {
		t.Fatalf("direct bridge/testdata consumers = %q, want %q", got, want)
	}
}

func TestConsumerSourceScanIncludesTestdata(t *testing.T) {
	root := t.TempDir()
	directory := filepath.Join(root, "testdata", "wrapper")
	if err := os.MkdirAll(directory, 0o700); err != nil {
		t.Fatal(err)
	}
	source := []byte("package wrapper\nimport _ \"" + deliveryDomainImportPath + "\"\n")
	if err := os.WriteFile(filepath.Join(directory, "wrapper.go"), source, 0o600); err != nil {
		t.Fatal(err)
	}
	consumers, err := scanModuleConsumerSources(root)
	if err != nil || len(consumers) != 1 {
		t.Fatalf("testdata consumers = %v, %v", consumers, err)
	}
}

func TestConsumerSourceScanDeduplicatesImportsPerFile(t *testing.T) {
	root := t.TempDir()
	source := []byte("package wrapper\nimport (\n_ \"" + deliveryDomainImportPath +
		"\"\n_ \"" + deliveryDomainImportPath + "\"\n)\n")
	if err := os.WriteFile(filepath.Join(root, "wrapper.go"), source, 0o600); err != nil {
		t.Fatal(err)
	}
	consumers, err := scanModuleConsumerSources(root)
	if err != nil || len(consumers) != 1 {
		t.Fatalf("duplicate imports produced duplicate consumers: %v, %v", consumers, err)
	}
}

func TestConsumerDiagnosticEscapesAndBoundsPaths(t *testing.T) {
	paths := []string{
		"normal.go", "line\nbreak\u202e.go",
		strings.Repeat("\x01", maxConsumerDiagnosticPathBytes+100), "omitted.go",
	}
	diagnostic := boundedConsumerDiagnostic(paths)
	if len(diagnostic) > maxConsumerDiagnosticBytes || strings.Contains(diagnostic, "\n") ||
		strings.Contains(diagnostic, "\u202e") || !strings.Contains(diagnostic, "count=4") ||
		!strings.Contains(diagnostic, `\n`) || !strings.Contains(diagnostic, `\u202e`) {
		t.Fatalf("consumer diagnostic is unsafe or unbounded: %q", diagnostic)
	}
}

func TestConsumerSourceScanRejectsDirectorySymlinks(t *testing.T) {
	root, target := t.TempDir(), t.TempDir()
	if err := os.Symlink(target, filepath.Join(root, "wrapper")); err != nil {
		t.Skipf("symlink creation is unavailable: %v", err)
	}
	if _, err := scanModuleConsumerSources(root); err == nil {
		t.Fatal("symlinked consumer directory bypassed the module scan")
	}
}
