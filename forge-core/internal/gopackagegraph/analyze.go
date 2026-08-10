package gopackagegraph

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"go/parser"
	"go/token"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

type Analysis struct {
	coverage     Coverage
	dependencies []Dependency
	diagnostics  []Diagnostic
	files        []File
	module       Module
	packages     []Package
}

func Analyze(
	ctx context.Context,
	plan *Plan,
	goMod RegularFile,
	goFiles []RegularFile,
) (*Analysis, error) {
	if plan == nil {
		return nil, fmt.Errorf("go package graph plan is absent")
	}
	if err := ctx.Err(); err != nil {
		return nil, fmt.Errorf("analyze Go package graph: %w", err)
	}
	modulePath, err := analyzeGoMod(plan, goMod)
	if err != nil {
		return nil, err
	}
	files, diagnostics, err := analyzeGoFiles(ctx, plan, goFiles)
	if err != nil {
		return nil, err
	}
	module := Module{
		Directory: plan.moduleDirectory, GoModBytes: plan.goMod.bytes,
		GoModContentSHA256: plan.goMod.sha256, GoModPath: plan.goMod.path,
		ModulePath: modulePath, NestedModules: cloneNestedModules(plan.nestedModules),
	}
	packages, err := derivePackages(files, module)
	if err != nil {
		return nil, err
	}
	dependencies, err := deriveDependencies(files, module, packages)
	if err != nil {
		return nil, err
	}
	coverage := plan.coverage
	coverage.RegularGoFilesParsed = int64(len(files))
	coverage.RegularGoFilesWithDiagnostics = int64(len(diagnostics))
	return &Analysis{
		coverage: coverage, dependencies: dependencies, diagnostics: diagnostics,
		files: files, module: module, packages: packages,
	}, nil
}

func (analysis *Analysis) Observation(
	observedAtUnixMS int64,
	producer Producer,
	source Source,
) (Observation, error) {
	if analysis == nil || observedAtUnixMS < 0 {
		return Observation{}, fmt.Errorf("analysis or observation time is invalid")
	}
	if !validProducer(producer) || !validSource(source) {
		return Observation{}, fmt.Errorf("producer or source binding is invalid")
	}
	return Observation{
		APIVersion: APIVersion, Canonicalization: Canonicalization,
		Coverage: analysis.coverage, Dependencies: cloneDependencies(analysis.dependencies),
		Diagnostics: append([]Diagnostic{}, analysis.diagnostics...),
		Files:       cloneFiles(analysis.files), Module: cloneModule(analysis.module),
		ObservedAtUnixMS: observedAtUnixMS, Packages: clonePackages(analysis.packages),
		Producer: producer, ProfileID: ProfileID, Source: source,
	}, nil
}

func analyzeGoMod(plan *Plan, file RegularFile) (string, error) {
	if err := verifyRegularFile(file, plan.goMod); err != nil {
		return "", fmt.Errorf("selected go.mod: %w", err)
	}
	if !utf8.Valid(file.Content) || !supportedSourceText(file.Content) {
		return "", fmt.Errorf("selected go.mod is not supported bounded UTF-8 text")
	}
	modulePath, err := parseModuleDirective(string(file.Content))
	if err != nil {
		return "", fmt.Errorf("selected go.mod: %w", err)
	}
	return modulePath, nil
}

func analyzeGoFiles(
	ctx context.Context,
	plan *Plan,
	values []RegularFile,
) ([]File, []Diagnostic, error) {
	if values == nil || len(values) != len(plan.goFilePaths) {
		return nil, nil, fmt.Errorf("regular Go file byte set does not match the read plan")
	}
	provided, err := indexReadFiles(plan, values)
	if err != nil {
		return nil, nil, err
	}
	files, diagnostics := make([]File, 0, len(values)), make([]Diagnostic, 0)
	occurrences := 0
	for _, selected := range plan.selected {
		if err := ctx.Err(); err != nil {
			return nil, nil, fmt.Errorf("analyze Go package graph: %w", err)
		}
		if selected.bytes > limits.GoFileBytes {
			diagnostics = append(diagnostics, Diagnostic{Code: "go_file_exceeds_parser_limit", Path: selected.path})
			continue
		}
		value, exists := provided[selected.path]
		if !exists {
			return nil, nil, fmt.Errorf("regular Go file %q is absent from supplied bytes", selected.path)
		}
		file, count, code, err := parseGoFile(selected, value)
		if err != nil {
			return nil, nil, err
		}
		if code != "" {
			diagnostics = append(diagnostics, Diagnostic{Code: code, Path: selected.path})
			continue
		}
		if count > maxImportOccurrences-occurrences {
			return nil, nil, fmt.Errorf("import occurrences exceed %d", maxImportOccurrences)
		}
		occurrences += count
		files = append(files, file)
	}
	if len(diagnostics) > maxDiagnostics {
		return nil, nil, fmt.Errorf("go file diagnostics exceed %d", maxDiagnostics)
	}
	sortDiagnostics(diagnostics)
	return files, diagnostics, nil
}

func sortDiagnostics(values []Diagnostic) {
	sort.Slice(values, func(i, j int) bool {
		if values[i].Path != values[j].Path {
			return values[i].Path < values[j].Path
		}
		return values[i].Code < values[j].Code
	})
}

func indexReadFiles(plan *Plan, values []RegularFile) (map[string]RegularFile, error) {
	provided := make(map[string]RegularFile, len(values))
	for index, value := range values {
		if value.Path != plan.goFilePaths[index] {
			return nil, fmt.Errorf("regular Go file byte set is not exact and path sorted")
		}
		provided[value.Path] = cloneRegularFile(value)
	}
	return provided, nil
}

func parseGoFile(expected selectedGoFile, value RegularFile) (File, int, string, error) {
	if err := verifyRegularFile(value, expected); err != nil {
		return File{}, 0, "", fmt.Errorf("regular Go file %q: %w", expected.path, err)
	}
	if !utf8.Valid(value.Content) {
		return File{}, 0, "go_file_invalid_utf8", nil
	}
	if !supportedSourceText(value.Content) {
		return File{}, 0, "go_file_unsupported_text", nil
	}
	parsed, parseErr := parser.ParseFile(
		token.NewFileSet(), expected.path, value.Content,
		parser.ImportsOnly|parser.SkipObjectResolution,
	)
	if parseErr != nil || parsed == nil || parsed.Name == nil {
		return File{}, 0, "go_file_parse_error", nil
	}
	if !validPackageIdentifier(parsed.Name.Name) || !validText(parsed.Name.Name) {
		return File{}, 0, "go_file_unsupported_text", nil
	}
	imports := make([]string, 0, len(parsed.Imports))
	for _, item := range parsed.Imports {
		importPath, err := strconv.Unquote(item.Path.Value)
		if err != nil {
			return File{}, 0, "go_file_parse_error", nil
		}
		if !validText(importPath) {
			return File{}, 0, "go_file_unsupported_text", nil
		}
		imports = append(imports, importPath)
	}
	sort.Strings(imports)
	imports = compactStrings(imports)
	if len(imports) > maxImportsPerFile {
		return File{}, 0, "go_file_import_limit_exceeded", nil
	}
	role := "compile"
	if strings.HasSuffix(expected.path, "_test.go") {
		role = "test"
	}
	return File{
		Bytes: expected.bytes, ContentSHA256: expected.sha256, Imports: imports,
		PackageName: parsed.Name.Name, Path: expected.path, Role: role,
	}, len(imports), "", nil
}

func verifyRegularFile(value RegularFile, expected selectedGoFile) error {
	if value.Path != expected.path || value.SHA256 != expected.sha256 ||
		int64(len(value.Content)) != expected.bytes || sha256Bytes(value.Content) != expected.sha256 {
		return fmt.Errorf("bytes do not match their source manifest entry")
	}
	return nil
}

func sha256Bytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}
