// Package yamlpath parses and resolves YAML path references used in workflow
// assets (eighth-wave-adr-decay.md §方向3: YAML 路径引用的机器解析).
//
// ForgeOS workflows reference YAML policy fragments with a compact syntax:
//
//	required_when: ../policies/modes.yml#workflow_depth.reviewer
//	gate_set:      ../policies/modes.yml#harness.gates
//
// Each reference has two parts separated by '#': a FILE path (relative to the
// referencing workflow) and a dot-separated FIELD path within that file's
// root object. This package parses those references into a structured form
// and resolves them by reading the target YAML (through the existing python
// yaml2json shim) and walking the resulting JSON tree.
//
// Zero external dependencies: file I/O uses the standard library; YAML→JSON
// conversion reuses the existing <root>/harness/yaml2json.py shim (which is
// already a runtime requirement of forge-core).
package yamlpath

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

// Ref is a parsed YAML path reference: a file path and a dot-separated field
// path into that file's root object.
type Ref struct {
	// File is the path to the YAML file, as authored in the reference
	// (e.g. "../policies/modes.yml"). It is relative to the referencing
	// workflow file and must be resolved to an absolute path at use time.
	File string
	// Path is the dot-separated field path into the YAML's root object
	// (e.g. "workflow_depth.reviewer" or "harness.gates").
	Path string
}

// String returns the canonical string form of the reference.
func (r Ref) String() string { return r.File + "#" + r.Path }

// Parse parses a YAML path reference string into its file and path components.
// The expected format is "<file>#<dot-separated-path>". A reference without
// '#' is invalid.
func Parse(ref string) (Ref, error) {
	file, path, ok := strings.Cut(ref, "#")
	if !ok {
		return Ref{}, fmt.Errorf("yamlpath: invalid reference %q: missing '#' separator", ref)
	}
	if file == "" {
		return Ref{}, fmt.Errorf("yamlpath: invalid reference %q: empty file path", ref)
	}
	if path == "" {
		return Ref{}, fmt.Errorf("yamlpath: invalid reference %q: empty field path", ref)
	}
	return Ref{File: file, Path: path}, nil
}

// MustParse parses a YAML path reference and panics on error. Use for
// hard-coded test references.
func MustParse(ref string) Ref {
	r, err := Parse(ref)
	if err != nil {
		panic(err)
	}
	return r
}

// ShimPath returns the absolute path to the yaml2json.py shim rooted at
// repoRoot. This is the transcoder that turns YAML into JSON so that
// forge-core's zero-dep JSON parser can read it.
func ShimPath(repoRoot string) string {
	return filepath.Join(repoRoot, "harness", "yaml2json.py")
}

// Resolve reads the YAML file referenced by r, transcodes it to JSON via
// the python yaml2json shim, walks the dot-separated field path, and returns
// the value at that path. repoRoot is the project root (where harness/ lives).
// baseDir is the directory of the referencing file (for resolving relative
// file paths in the reference).
//
// The returned value is a Go value decoded from JSON: nil, bool, float64,
// string, []any, or map[string]any. The caller should type-assert as needed.
//
// Errors: missing shim, missing/invalid file, unresolvable path, or a YAML
// format that the shim cannot transcode.
func Resolve(repoRoot, ref string, baseDir string) (any, error) {
	r, err := Parse(ref)
	if err != nil {
		return nil, err
	}
	return resolveRef(repoRoot, r, baseDir)
}

// resolveRef is the inner resolver after parsing.
func resolveRef(repoRoot string, r Ref, baseDir string) (any, error) {
	// Resolve the YAML file path: relative to baseDir, then fallback to repoRoot.
	absFile := filepath.Join(baseDir, r.File)
	// Make absolute: if it still contains a parent-dir reference, resolve it
	// relative to repoRoot.
	if !filepath.IsAbs(absFile) {
		absFile = filepath.Join(repoRoot, r.File)
	}
	absFile = filepath.Clean(absFile)

	// Transcode YAML→JSON via the python shim.
	shim := ShimPath(repoRoot)
	if _, err := os.Stat(shim); err != nil {
		return nil, fmt.Errorf("yamlpath: yaml2json shim not found at %s: %w", shim, err)
	}
	if _, err := os.Stat(absFile); err != nil {
		return nil, fmt.Errorf("yamlpath: YAML file not found at %s: %w", absFile, err)
	}

	out, err := exec.Command("python3", shim, absFile).Output()
	if err != nil {
		return nil, fmt.Errorf("yamlpath: transcode %s: %w", absFile, err)
	}

	// Decode the JSON into a generic tree.
	var root any
	if err := json.Unmarshal(out, &root); err != nil {
		return nil, fmt.Errorf("yamlpath: decode JSON from %s: %w", absFile, err)
	}

	// Walk the dot-separated path.
	return walkPath(root, strings.Split(r.Path, "."))
}

// walkPath walks a dot-separated field path through a decoded JSON tree.
// It supports map[string]any (objects) and []any indexed by integer string
// (e.g. "nodes.0.name").
func walkPath(current any, segments []string) (any, error) {
	for i, seg := range segments {
		if current == nil {
			return nil, fmt.Errorf("yamlpath: cannot resolve segment %q (path stopped at %q): value is nil",
				seg, strings.Join(segments[:i], "."))
		}
		switch v := current.(type) {
		case map[string]any:
			val, ok := v[seg]
			if !ok {
				return nil, fmt.Errorf("yamlpath: key %q not found at %q (available keys: %v)",
					seg, strings.Join(segments[:i+1], "."), mapKeys(v))
			}
			current = val
		case []any:
			idx, err := parseIndex(seg)
			if err != nil {
				return nil, fmt.Errorf("yamlpath: cannot index array at %q with non-integer %q: %w",
					strings.Join(segments[:i+1], "."), seg, err)
			}
			if idx < 0 || idx >= len(v) {
				return nil, fmt.Errorf("yamlpath: index %d out of bounds at %q (array length %d)",
					idx, strings.Join(segments[:i+1], "."), len(v))
			}
			current = v[idx]
		default:
			return nil, fmt.Errorf("yamlpath: cannot descend into %T at %q (segment %q)",
				current, strings.Join(segments[:i], "."), seg)
		}
	}
	return current, nil
}

// mapKeys returns the keys of a map for error messages.
func mapKeys(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	return keys
}

// parseIndex parses a string as a non-negative integer array index.
// Rejects empty strings and values that overflow int.
func parseIndex(s string) (int, error) {
	if s == "" {
		return 0, fmt.Errorf("yamlpath: empty index string")
	}
	var n int
	for _, c := range s {
		if c < '0' || c > '9' {
			return 0, fmt.Errorf("yamlpath: not a valid index: %q", s)
		}
		next := n*10 + int(c-'0')
		// Check for overflow: if next wrapped around or became negative.
		if next < 0 || next/10 != n {
			return 0, fmt.Errorf("yamlpath: index overflow: %q", s)
		}
		n = next
	}
	return n, nil
}
