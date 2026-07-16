// detect_parsers.go — structural and semantic project scanning for `forge detect`.
// Split from detect.go to keep each file under the harness size budget.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// detectProject scans root for structural and semantic project signals.
func detectProject(root string) projectProfile {
	p := projectProfile{Lifecycle: "mvp", Mode: "balanced"}
	p.Language, p.Indicators = detectLanguage(root)
	p.HasTests, p.Indicators = detectTests(root, p.Language, p.Indicators)
	p.HasCI, p.Indicators = detectCI(root, p.Indicators)

	// v1.5: Semantic manifest parsing — additive, never replaces structural detection.
	// Each parser degrades gracefully (empty fields) on missing/corrupt files.
	switch p.Language {
	case "go":
		p.GoModulePath, p.GoVersion, p.Indicators = parseGoMod(root, p.Indicators)
	case "node":
		var buildSc, testSc bool
		var deps int
		buildSc, testSc, deps, p.Indicators = parsePackageJSON(root, p.Indicators)
		p.HasBuildScript = buildSc
		p.HasTestScript = testSc
		p.DepsCount = deps
	case "python":
		p.BuildBackend, p.PythonVersion, p.Indicators = parsePyprojectToml(root, p.Indicators)
	case "rust":
		p.CrateName, p.RustEdition, p.Indicators = parseCargoToml(root, p.Indicators)
	}

	p.Lifecycle, p.Mode, p.Indicators = applyProjectYML(root, p.Lifecycle, p.Mode, p.Indicators)
	if len(p.Indicators) == 0 {
		p.Indicators = []string{"no structural indicators found"}
	}
	return p
}

// ── Semantic manifest parsers (v1.5) ──────────────────────────────────────
//
// All three share the same honesty contract:
//   - Missing file → empty fields + no indicator → no change to profile.
//   - Corrupt/unparseable content → empty fields + "parse error" indicator,
//     but NEVER a crash — the parsers always return something.
//   - Successful parse → populated fields + indicator listing what was found.

// parseGoMod reads go.mod for the `module` directive (module path) and the `go`
// directive (Go language version). forge-core is zero-dep so this is a line
// scanner, the same approach as readProjectYML. Returns the found module path,
// Go version, and updated indicator slice.
func parseGoMod(root string, ind []string) (modulePath, goVersion string, indicators []string) {
	data, err := os.ReadFile(filepath.Join(root, "go.mod"))
	if err != nil {
		return "", "", ind // missing or unreadable: graceful degradation
	}
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if after, ok := strings.CutPrefix(trimmed, "module "); ok {
			modulePath = strings.TrimSpace(strings.Split(after, "//")[0])
			modulePath = strings.TrimSpace(strings.Split(modulePath, "#")[0])
			ind = append(ind, fmt.Sprintf("go.mod module=%s", modulePath))
		}
		if after, ok := strings.CutPrefix(trimmed, "go "); ok {
			goVersion = strings.TrimSpace(strings.Split(after, "//")[0])
			goVersion = strings.TrimSpace(strings.Split(goVersion, "#")[0])
			ind = append(ind, fmt.Sprintf("go.mod go=%s", goVersion))
		}
	}
	return modulePath, goVersion, ind
}

// parsePackageJSON reads package.json for the `scripts.build`, `scripts.test`,
// and dependency count fields. Uses Go stdlib's json decoder (the only JSON lib
// we have — zero-dep, no external imports). Returns build-script presence,
// test-script presence, dependency count, and updated indicators.
func parsePackageJSON(root string, ind []string) (hasBuild, hasTest bool, deps int, indicators []string) {
	data, err := os.ReadFile(filepath.Join(root, "package.json"))
	if err != nil {
		return false, false, 0, ind // missing: graceful degradation
	}
	var parsed struct {
		Scripts         map[string]string `json:"scripts"`
		Dependencies    map[string]string `json:"dependencies"`
		DevDependencies map[string]string `json:"devDependencies"`
	}
	if err := json.Unmarshal(data, &parsed); err != nil {
		return false, false, 0, append(ind, "package.json parse error") // corrupt: note + degrade
	}
	_, hasBuild = parsed.Scripts["build"]
	_, hasTest = parsed.Scripts["test"]
	deps = len(parsed.Dependencies) + len(parsed.DevDependencies)
	ind = append(ind, fmt.Sprintf("package.json scripts: build=%s test=%s deps=%d",
		boolStr(hasBuild), boolStr(hasTest), deps))
	return hasBuild, hasTest, deps, ind
}

// parsePyprojectToml reads pyproject.toml for the `[build-system] build-backend`
// and `[project] requires-python` fields. forge-core has no TOML lib, so this is
// a lightweight section-scanner that finds a `[section]` header, then reads
// key = value lines until the next `[section]` or EOF. Returns build-backend
// name, Python version constraint, and updated indicators.
func parsePyprojectToml(root string, ind []string) (buildBackend, pythonVersion string, indicators []string) {
	data, err := os.ReadFile(filepath.Join(root, "pyproject.toml"))
	if err != nil {
		return "", "", ind // missing: graceful degradation
	}
	var currentSection string
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			continue
		}
		if strings.HasPrefix(trimmed, "[") && strings.HasSuffix(trimmed, "]") {
			currentSection = strings.TrimSpace(trimmed[1 : len(trimmed)-1])
			continue
		}
		switch currentSection {
		case "build-system":
			if after, ok := strings.CutPrefix(trimmed, "build-backend = "); ok {
				buildBackend = strings.Trim(strings.Split(after, "#")[0], "\" '")
				ind = append(ind, fmt.Sprintf("pyproject.toml build-backend=%s", buildBackend))
			}
		case "project":
			if after, ok := strings.CutPrefix(trimmed, "requires-python = "); ok {
				pythonVersion = strings.Trim(strings.Split(after, "#")[0], "\" '")
				ind = append(ind, fmt.Sprintf("pyproject.toml requires-python=%s", pythonVersion))
			}
		}
	}
	return buildBackend, pythonVersion, ind
}

// parseCargoToml reads Cargo.toml for the `[package] name` (crate name) and
// `[package] edition` (Rust edition: 2015|2018|2021|2024) fields. Uses the
// same zero-dep section-scanner pattern as parsePyprojectToml: finds a
// `[section]` header, reads key = value lines until the next `[section]` or
// EOF. Returns crate name, edition, and updated indicators.
func parseCargoToml(root string, ind []string) (crateName, edition string, indicators []string) {
	data, err := os.ReadFile(filepath.Join(root, "Cargo.toml"))
	if err != nil {
		return "", "", ind // missing: graceful degradation
	}
	var currentSection string
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			continue
		}
		if strings.HasPrefix(trimmed, "[") && strings.HasSuffix(trimmed, "]") {
			currentSection = strings.TrimSpace(trimmed[1 : len(trimmed)-1])
			continue
		}
		if currentSection != "package" {
			continue
		}
		if after, ok := strings.CutPrefix(trimmed, "name = "); ok && crateName == "" {
			crateName = strings.Trim(strings.Split(after, "#")[0], "\" '")
			ind = append(ind, fmt.Sprintf("Cargo.toml name=%s", crateName))
		}
		if after, ok := strings.CutPrefix(trimmed, "edition = "); ok && edition == "" {
			edition = strings.Trim(strings.Split(after, "#")[0], "\" '")
			ind = append(ind, fmt.Sprintf("Cargo.toml edition=%s", edition))
		}
	}
	return crateName, edition, ind
}

// detectLanguage returns the primary language and any indicator strings.
func detectLanguage(root string) (lang string, indicators []string) {
	type manifest struct{ file, lang string }
	manifests := []manifest{
		{"go.mod", "go"},
		{"package.json", "node"},
		{"pyproject.toml", "python"},
		{"requirements.txt", "python"},
		{"Cargo.toml", "rust"},
	}
	for _, m := range manifests {
		if fileExists(filepath.Join(root, m.file)) {
			return m.lang, []string{m.file + " found"}
		}
	}
	return "unknown", nil
}

// detectTests checks whether the project has test files for the given language.
func detectTests(root, lang string, ind []string) (hasTests bool, indicators []string) {
	globs := map[string][]string{
		"go":     {"*_test.go"},
		"node":   {"*.test.ts", "*.test.js", "*.spec.ts", "*.spec.js"},
		"python": {"test_*.py", "*_test.py"},
		"rust":   {"*_test.rs", "tests/*.rs"},
	}
	for _, g := range globs[lang] {
		if dirHasGlob(root, g) {
			return true, append(ind, g+" files found")
		}
	}
	return false, ind
}

// detectCI checks for common CI configuration files.
func detectCI(root string, ind []string) (hasCI bool, indicators []string) {
	ciPaths := []string{
		filepath.Join(root, ".github", "workflows"),
		filepath.Join(root, ".gitlab-ci.yml"),
		filepath.Join(root, "Jenkinsfile"),
	}
	for _, p := range ciPaths {
		if fileExists(p) {
			return true, append(ind, "CI config found")
		}
	}
	return false, ind
}

// applyProjectYML reads lifecycle and mode from .agent/project.yml if present.
func applyProjectYML(root, lifecycle, mode string, ind []string) (string, string, []string) {
	lc, m, ok := readProjectYML(filepath.Join(root, ".agent", "project.yml"))
	if !ok {
		return lifecycle, mode, ind
	}
	if lc != "" {
		lifecycle = lc
		ind = append(ind, fmt.Sprintf("project.yml lifecycle=%s", lc))
	}
	if m != "" {
		mode = m
		ind = append(ind, fmt.Sprintf("project.yml mode=%s", m))
	}
	return lifecycle, mode, ind
}

// suggestWorkflow maps a projectProfile to a workflow + flags recommendation.
// The mode/lifecycle from project.yml (or defaults) always win — semantic fields
// only enrich the reason text.
func suggestWorkflow(p projectProfile) workflowSuggestion {
	mode := p.Mode
	lifecycle := p.Lifecycle

	// Unknown language with no tests → likely greenfield; start with discovery.
	if p.Language == "unknown" && !p.HasTests {
		return workflowSuggestion{
			Workflow:  "discover",
			Mode:      mode,
			Lifecycle: lifecycle,
			Reason:    "no language manifest + no tests detected → start with requirement discovery",
		}
	}

	// Build the reason string from available signals.
	return workflowSuggestion{
		Workflow:  "evolve",
		Mode:      mode,
		Lifecycle: lifecycle,
		Reason:    buildSuggestionReason(p),
	}
}

// buildSuggestionReason assembles a human-readable one-liner explaining why
// a workflow was suggested, using all available structural + semantic signals.
func buildSuggestionReason(p projectProfile) string {
	var parts []string

	lang := p.Language
	if lang == "" {
		lang = "unknown"
	}
	parts = append(parts, lang)

	if p.GoModulePath != "" {
		parts = append(parts, fmt.Sprintf("mod=%s", shortenModule(p.GoModulePath)))
	}
	if p.HasTestScript {
		parts = append(parts, "test-script")
	} else if p.HasBuildScript {
		parts = append(parts, "build-script")
	}
	if p.DepsCount > 0 {
		parts = append(parts, fmt.Sprintf("%d-deps", p.DepsCount))
	}
	if p.CrateName != "" {
		parts = append(parts, fmt.Sprintf("crate=%s", p.CrateName))
	}
	if p.BuildBackend != "" {
		parts = append(parts, fmt.Sprintf("backend=%s", p.BuildBackend))
	}

	if p.HasTests {
		parts = append(parts, "tests-found")
	} else {
		parts = append(parts, "no-tests")
	}

	if p.HasCI {
		parts = append(parts, "ci")
	} else {
		parts = append(parts, "no-ci")
	}

	return fmt.Sprintf("iterative improvement loop (%s)", strings.Join(parts, " | "))
}

// shortenModule trims a full go module path for display.
func shortenModule(mod string) string {
	parts := strings.Split(mod, "/")
	if len(parts) > 3 {
		return strings.Join(parts[:3], "/")
	}
	return mod
}

// fileExists reports whether the path exists (file or directory).
func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// dirHasGlob reports whether any file matching pattern exists anywhere under root.
func dirHasGlob(root, pattern string) bool {
	found := false
	_ = filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil || found {
			return nil
		}
		if d.IsDir() && (d.Name() == ".git" || d.Name() == "vendor" || d.Name() == "node_modules") {
			return filepath.SkipDir
		}
		if ok, _ := filepath.Match(pattern, d.Name()); ok {
			found = true
		}
		return nil
	})
	return found
}

// readProjectYML reads lifecycle and mode from a project.yml file.
// Returns ("", "", false) when the file is missing or unreadable.
func readProjectYML(path string) (lifecycle, mode string, ok bool) {
	f, err := os.Open(path)
	if err != nil {
		return "", "", false
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if after, found := strings.CutPrefix(line, "lifecycle:"); found {
			lifecycle = strings.TrimSpace(strings.Split(after, "#")[0])
		}
		if after, found := strings.CutPrefix(line, "mode:"); found {
			mode = strings.TrimSpace(strings.Split(after, "#")[0])
		}
	}
	return lifecycle, mode, true
}
