// Package gopackagegraph derives a bounded lexical Go package/import graph
// from caller-supplied, source-manifest-bound bytes. It performs no filesystem,
// Git, Go toolchain, dependency-availability, or selected-build inspection.
package gopackagegraph

const (
	APIVersion       = "forgeos.go-package-dependency-graph-observation/v1"
	Canonicalization = "forgeos.canonical-json/v1"
	ProfileID        = "selected-go-module-lexical-dependency-graph-v1"

	maxNestedModules     = 1_024
	maxImportsPerFile    = 1_024
	maxImportOccurrences = 65_536
	maxPackages          = 16_384
	maxEdges             = 65_536
	maxDiagnostics       = 16_384
)

type Bounds struct {
	AggregateParserBytes int64
	GoFileBytes          int64
	GoFiles              int
	GoModBytes           int64
}

var limits = Bounds{
	AggregateParserBytes: 64 << 20,
	GoFileBytes:          4 << 20,
	GoFiles:              16_384,
	GoModBytes:           1 << 20,
}

func ReadLimits() Bounds { return limits }

type SourceEntry struct {
	Bytes         int64
	ContentSHA256 *string
	Kind          string
	Path          string
}

type RegularFile struct {
	Content []byte
	Path    string
	SHA256  string
}

type Coverage struct {
	GoEntriesExcludedByNestedModule int64 `json:"go_entries_excluded_by_nested_module"`
	GoEntriesExcludedNonregular     int64 `json:"go_entries_excluded_nonregular"`
	GoEntriesInSelectedSubtree      int64 `json:"go_entries_in_selected_subtree"`
	RegularGoFilesParsed            int64 `json:"regular_go_files_parsed"`
	RegularGoFilesSelected          int64 `json:"regular_go_files_selected"`
	RegularGoFilesWithDiagnostics   int64 `json:"regular_go_files_with_diagnostics"`
}

type Dependency struct {
	FromDirectory     string   `json:"from_directory"`
	FromPackageName   string   `json:"from_package_name"`
	ImportPath        string   `json:"import_path"`
	Relation          string   `json:"relation"`
	Resolution        string   `json:"resolution"`
	ResolutionDetail  *string  `json:"resolution_detail"`
	Role              string   `json:"role"`
	SourcePaths       []string `json:"source_paths"`
	TargetDirectory   *string  `json:"target_directory"`
	TargetPackageName *string  `json:"target_package_name"`
}

type Diagnostic struct {
	Code string `json:"code"`
	Path string `json:"path"`
}

type File struct {
	Bytes         int64    `json:"bytes"`
	ContentSHA256 string   `json:"content_sha256"`
	Imports       []string `json:"imports"`
	PackageName   string   `json:"package_name"`
	Path          string   `json:"path"`
	Role          string   `json:"role"`
}

type Module struct {
	Directory          string         `json:"directory"`
	GoModBytes         int64          `json:"go_mod_bytes"`
	GoModContentSHA256 string         `json:"go_mod_content_sha256"`
	GoModPath          string         `json:"go_mod_path"`
	ModulePath         string         `json:"module_path"`
	NestedModules      []NestedModule `json:"nested_modules"`
}

type NestedModule struct {
	Directory string `json:"directory"`
	GoModPath string `json:"go_mod_path"`
	Kind      string `json:"kind"`
}

type Package struct {
	CompileFiles []string `json:"compile_files"`
	Directory    string   `json:"directory"`
	ImportPath   *string  `json:"import_path"`
	Name         string   `json:"name"`
	TestFiles    []string `json:"test_files"`
}

type Producer struct {
	ParametersSHA256 string `json:"parameters_sha256"`
	ProducerID       string `json:"producer_id"`
	ProducerType     string `json:"producer_type"`
	ProducerVersion  string `json:"producer_version"`
	RunID            string `json:"run_id"`
}

type Source struct {
	SourceRevision   string `json:"source_revision"`
	SourceTreeSHA256 string `json:"source_tree_sha256"`
}

type Observation struct {
	APIVersion       string       `json:"api_version"`
	Canonicalization string       `json:"canonicalization"`
	Coverage         Coverage     `json:"coverage"`
	Dependencies     []Dependency `json:"dependencies"`
	Diagnostics      []Diagnostic `json:"diagnostics"`
	Files            []File       `json:"files"`
	Module           Module       `json:"module"`
	ObservedAtUnixMS int64        `json:"observed_at_unix_ms"`
	Packages         []Package    `json:"packages"`
	Producer         Producer     `json:"producer"`
	ProfileID        string       `json:"profile_id"`
	Source           Source       `json:"source"`
}
