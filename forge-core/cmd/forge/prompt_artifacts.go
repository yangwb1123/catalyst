// prompt_artifacts.go — the workflow-DECLARED artifact-context lanes of
// buildPrompt: `emits` (prior phases' declared output files), `writes_adr`
// (the ADR target directory for design-stage phases), and the paired
// `uses_template` / `secondary_template` AI-SDLC template references
// (review.yml's performance-reliability-review phase pairs
// .ai/prompts/05-performance-review.md with .../06-production-readiness.md —
// one phase, two review dimensions). Unlike prompt_context.go's
// appendFeedbackLanes lanes, these are NEVER gated on FreshContext — see
// appendArtifactContext's doc comment for why. Split out of prompt_context.go
// to keep both files under the volume budget (that file's package doc
// explains the same split rationale for prompt_memory.go).
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
)

var writesADRKnownModes = map[string]bool{
	"explorer": true, "balanced": true, "engineering": true, "cto": true,
}

var writesADRKnownLifecycles = map[string]bool{
	"idea": true, "mvp": true, "growth": true, "production": true,
}

type writesADRCondition struct {
	modes map[string]bool
}

// parseWritesADRCondition implements the condition grammar authored by the
// workflow schema: `mode in [engineering, cto]`. Unknown syntax or values fail
// closed; a typo must never silently grant an ADR write scope.
func parseWritesADRCondition(raw string) (writesADRCondition, error) {
	expr := strings.TrimSpace(raw)
	rest, ok := consumeConditionKeyword(expr, "mode")
	if !ok {
		return writesADRCondition{}, fmt.Errorf("unsupported condition %q", raw)
	}
	rest, ok = consumeConditionKeyword(rest, "in")
	if !ok || len(rest) < 2 || rest[0] != '[' || rest[len(rest)-1] != ']' {
		return writesADRCondition{}, fmt.Errorf("unsupported condition %q", raw)
	}
	body := strings.TrimSpace(rest[1 : len(rest)-1])
	if body == "" {
		return writesADRCondition{}, fmt.Errorf("condition %q has no modes", raw)
	}
	parsed := writesADRCondition{modes: make(map[string]bool)}
	for _, item := range strings.Split(body, ",") {
		value := strings.TrimSpace(item)
		if !writesADRKnownModes[value] {
			return writesADRCondition{}, fmt.Errorf("condition %q names unknown mode %q", raw, value)
		}
		parsed.modes[value] = true
	}
	return parsed, nil
}

func consumeConditionKeyword(input, keyword string) (string, bool) {
	if !strings.HasPrefix(input, keyword) || len(input) == len(keyword) {
		return "", false
	}
	switch input[len(keyword)] {
	case ' ', '\t', '\r', '\n':
		return strings.TrimSpace(input[len(keyword):]), true
	default:
		return "", false
	}
}

func (c writesADRCondition) matches(runMode string) bool {
	return c.modes[runMode]
}

// effectiveWritesADR validates a declaration and combines it with the
// authoritative mode/lifecycle policy. A declaration may narrow a normal mode,
// while production (or the policy's unknown-input fail-safe) can force ADR on.
// The returned copy carries a normalized repo-relative target.
func effectiveWritesADR(repoRoot, runMode, lifecycle string, wa *asset.WritesADR) (*asset.WritesADR, string) {
	if wa == nil {
		return nil, ""
	}
	condition, err := parseWritesADRCondition(wa.Condition)
	if err != nil {
		return nil, err.Error()
	}
	_, relTarget, err := containedADRTarget(repoRoot, wa.Target)
	if err != nil {
		return nil, err.Error()
	}
	policy := mode.Effective(runMode, lifecycle)
	if !policy.ADR {
		return nil, fmt.Sprintf("effective policy mode=%q lifecycle=%q does not allow ADR", runMode, lifecycle)
	}
	forced := lifecycle == "production" || !writesADRKnownModes[runMode] || !writesADRKnownLifecycles[lifecycle]
	if !condition.matches(runMode) && !forced {
		return nil, fmt.Sprintf("condition %q is false for mode %q", wa.Condition, runMode)
	}
	authorized := *wa
	authorized.Target = relTarget
	return &authorized, ""
}

// containedADRTarget resolves an ADR directory without mutating the filesystem.
// It rejects lexical traversal, absolute paths outside the repo, the repo root
// itself, and escapes through any existing symlink prefix.
func containedADRTarget(repoRoot, target string) (absolute, relative string, err error) {
	if strings.TrimSpace(target) == "" {
		return "", "", fmt.Errorf("writes_adr target is empty")
	}
	absolute, relative, err = containedRepoPath(repoRoot, target)
	if err != nil {
		return "", "", fmt.Errorf("writes_adr target %q: %w", target, err)
	}
	relative = filepath.ToSlash(relative)
	if !safeADRRelativePath(relative) {
		return "", "", fmt.Errorf("writes_adr target %q contains unsafe permission-pattern characters", target)
	}
	relative = strings.TrimSuffix(relative, "/") + "/"
	return absolute, relative, nil
}

func safeADRRelativePath(path string) bool {
	for _, ch := range path {
		switch {
		case ch >= 'a' && ch <= 'z':
		case ch >= 'A' && ch <= 'Z':
		case ch >= '0' && ch <= '9':
		case ch == '/', ch == '.', ch == '_', ch == '-':
		default:
			return false
		}
	}
	return path != ""
}

// containedRepoPath is the shared lexical + symlink-aware containment check for
// runtime-controlled paths. It is read-only and requires a strict descendant,
// never the repository root itself.
func containedRepoPath(repoRoot, target string) (absolute, relative string, err error) {
	if repoRoot == "" {
		repoRoot = "."
	}
	rootAbs, err := filepath.Abs(repoRoot)
	if err != nil {
		return "", "", fmt.Errorf("resolve repo root: %w", err)
	}
	rootReal, err := filepath.EvalSymlinks(rootAbs)
	if err != nil {
		return "", "", fmt.Errorf("resolve repo root %q: %w", rootAbs, err)
	}
	targetAbs := target
	if !filepath.IsAbs(targetAbs) {
		targetAbs = filepath.Join(rootAbs, targetAbs)
	}
	targetAbs = filepath.Clean(targetAbs)
	lexicalRel, ok := strictRelativeTo(rootAbs, targetAbs)
	if !ok {
		return "", "", fmt.Errorf("path escapes repo %q", rootAbs)
	}
	targetReal, err := resolveExistingPrefix(targetAbs)
	if err != nil {
		return "", "", fmt.Errorf("resolve target: %w", err)
	}
	if _, ok := strictRelativeTo(rootReal, targetReal); !ok {
		return "", "", fmt.Errorf("path escapes repo through a symlink")
	}
	return targetAbs, lexicalRel, nil
}

// validateReadonlyEmitIdentity rejects in-repository alias tricks before an
// exact Edit(path) grant is constructed. Existing components below the repo
// root must be real directories; an existing emit must be a single-link file.
func validateReadonlyEmitIdentity(repoRoot, absolute string) error {
	root, err := filepath.Abs(repoRoot)
	if err != nil {
		return fmt.Errorf("resolve repository root: %w", err)
	}
	relative, err := filepath.Rel(root, absolute)
	if err != nil {
		return fmt.Errorf("resolve emit path: %w", err)
	}
	current := root
	parts := strings.Split(filepath.Clean(relative), string(filepath.Separator))
	for i, part := range parts {
		current = filepath.Join(current, part)
		info, statErr := os.Lstat(current)
		if os.IsNotExist(statErr) {
			return nil
		}
		if statErr != nil {
			return statErr
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("path component %q must not be a symlink", current)
		}
		if i < len(parts)-1 && !info.IsDir() {
			return fmt.Errorf("path component %q must be a directory", current)
		}
		if i == len(parts)-1 && (!info.Mode().IsRegular() || !releaseRegularSingleLink(info)) {
			return fmt.Errorf("existing emit must be a regular single-link file")
		}
	}
	return nil
}

// resolveExistingPrefix evaluates symlinks in the nearest existing ancestor,
// then appends the still-missing suffix. Lstat distinguishes a dangling symlink
// (which must be rejected) from a genuinely missing path component.
func resolveExistingPrefix(path string) (string, error) {
	current := filepath.Clean(path)
	var missing []string
	for {
		if _, err := os.Lstat(current); err == nil {
			resolved, err := filepath.EvalSymlinks(current)
			if err != nil {
				return "", err
			}
			for i := len(missing) - 1; i >= 0; i-- {
				resolved = filepath.Join(resolved, missing[i])
			}
			return filepath.Clean(resolved), nil
		} else if !os.IsNotExist(err) {
			return "", err
		}
		parent := filepath.Dir(current)
		if parent == current {
			return "", fmt.Errorf("no existing ancestor for %q", path)
		}
		missing = append(missing, filepath.Base(current))
		current = parent
	}
}

func strictRelativeTo(base, candidate string) (string, bool) {
	rel, err := filepath.Rel(base, candidate)
	if err != nil || rel == "." || filepath.IsAbs(rel) {
		return "", false
	}
	if rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", false
	}
	return rel, true
}

// emitsContext reads the content of files that prior phases declared via the
// `emits` field (asset-runtime-gap.md §1.3). It reads each file path relative
// to repoRoot and returns context blocks for any that exist. A missing/unreadable
// file is not an error (the phase may not have run yet, or a prior crash left an
// intermittent artifact) but WARNs via logln — the same diagnostic symmetry
// usesTemplateContext gives a missing template — when logln is non-nil, so a
// caller that wants the signal can surface it without the phase being blocked.
// The returned blocks are prefixed with a [context:emit] marker so the agent can
// distinguish system-provided artifact content from instructions. Pure-adjacent:
// the only IO is os.ReadFile for each emit path (+ the optional logln callback).
func emitsContext(repoRoot string, emits []string, logln func(string)) []string {
	if len(emits) == 0 {
		return nil
	}
	var blocks []string
	for _, path := range emits {
		fullPath, relative, err := containedRepoPath(repoRoot, path)
		if err != nil {
			if logln != nil {
				logln(fmt.Sprintf("forge: WARNING emits %q rejected (%v)", path, err))
			}
			continue
		}
		data, err := os.ReadFile(fullPath)
		if err != nil {
			if logln != nil {
				logln(fmt.Sprintf("forge: WARNING emits %q not found (%v)", fullPath, err))
			}
			continue // missing file is not an error — phase may not have run
		}
		content := strings.TrimSpace(string(data))
		if content == "" {
			continue
		}
		blocks = append(blocks, contextMarker("emit:"+filepath.ToSlash(relative), content))
	}
	return blocks
}

// templateContext reads an AI-SDLC template file and returns a context block
// with a [context:template] marker. Shared by usesTemplateContext and
// secondaryTemplateContext below — the pair mirrors the two template fields
// review.yml's performance-reliability-review phase pairs (uses_template +
// secondary_template on the SAME phase, e.g. 05-performance-review.md
// alongside 06-production-readiness.md). An empty template path (the
// default — no reference) returns "" silently. A missing template file
// WARNs via stderr but does NOT block: the phase still runs, just without
// the specialized dimension guidance. fieldName (e.g. "uses_template" /
// "secondary_template") only feeds the WARNING message so it names WHICH
// field's path went missing. The template path is relative to repoRoot
// (e.g. ".ai/prompts/02-security-rfc-review.md").
func templateContext(repoRoot, fieldName, templatePath string) string {
	if templatePath == "" {
		return ""
	}
	fullPath, relative, err := containedRepoPath(repoRoot, templatePath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge: WARNING %s %q rejected (%v)\n", fieldName, templatePath, err)
		return ""
	}
	data, err := os.ReadFile(fullPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge: WARNING %s %q not found (%v)\n", fieldName, fullPath, err)
		return ""
	}
	content := strings.TrimSpace(string(data))
	if content == "" {
		return ""
	}
	return contextMarker("template:"+filepath.ToSlash(relative), content)
}

// usesTemplateContext reads the AI-SDLC template file referenced by a phase's
// uses_template field. See templateContext for the shared behavior.
func usesTemplateContext(repoRoot, templatePath string) string {
	return templateContext(repoRoot, "uses_template", templatePath)
}

// secondaryTemplateContext reads the AI-SDLC template file referenced by a
// phase's secondary_template field — the second, optional review dimension
// paired alongside uses_template on the same phase. See templateContext for
// the shared behavior.
func secondaryTemplateContext(repoRoot, templatePath string) string {
	return templateContext(repoRoot, "secondary_template", templatePath)
}

// writesAdrContext returns a context block that tells the agent to produce an
// Architecture Decision Record (ADR) in the declared target directory. It is
// only meaningful for design-stage phases that declare writes_adr
// (design.yml's solution-architect). The context block includes:
//   - The ADR target directory (e.g. docs/adr/)
//   - The file naming convention (ADR-XXXX-title.md)
//   - The ADR structure (Context → Decision → Consequences)
//
// We scan the existing ADR directory to determine the next sequence number.
// This helper is deliberately read-only: creating the directory while building
// a prompt made dry-run-like inspection mutate the repo. The command executor
// authorizes condition/mode/lifecycle before passing WritesADR here; this
// defense-in-depth layer re-validates condition syntax and target containment.
func writesAdrContext(repoRoot string, wa *asset.WritesADR) string {
	if wa == nil || wa.Target == "" {
		return ""
	}
	if _, err := parseWritesADRCondition(wa.Condition); err != nil {
		fmt.Fprintf(os.Stderr, "forge: WARNING writes_adr %v\n", err)
		return ""
	}
	targetDir, relTarget, err := containedADRTarget(repoRoot, wa.Target)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge: WARNING %v\n", err)
		return ""
	}
	// Count existing ADRs to suggest the next sequence number.
	nextSeq := nextADRSequence(targetDir)
	block := fmt.Sprintf(`You are expected to produce an Architecture Decision Record (ADR) for your design.

Target directory: %s
Naming convention: ADR-%%04d-title.md (next available: ADR-%04d-*.md)

Structure your ADR with these sections:
  - Title: A short, descriptive name
  - Status: Proposed | Accepted | Deprecated | Superseded
  - Context: Why this decision is needed, what forces are at play
  - Decision: The chosen approach and rationale
  - Consequences: Trade-offs, risks, and follow-up work

Review existing ADRs in the target directory for style reference.`, relTarget, nextSeq)
	return contextMarker("writes_adr", block)
}

// nextADRSequence scans the ADR target directory for existing ADR-XXXX-*.md
// files and returns the next available sequence number (max+1). Returns 1
// if no ADRs exist or the directory cannot be read.
func nextADRSequence(dir string) int {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return 1
	}
	maxNum := 0
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		// Match "ADR-XXXX-*.md" or "XXXX-*.md" (the repo's existing ADRs).
		// The canonical format is "ADR-0004-title.md" but existing files
		// may use "NUMBER-title.md" (e.g. "0001-decision.md"). Accept both.
		if !strings.HasSuffix(name, ".md") {
			continue
		}
		num, ok := adrSequenceNumber(name)
		if !ok {
			continue
		}
		if num > maxNum {
			maxNum = num
		}
	}
	return maxNum + 1
}

// appendArtifactContext appends the emitted-artifact, writes_adr context,
// uses_template, and secondary_template context lanes onto ctx. Unlike
// appendFeedbackLanes, these are NEVER gated on FreshContext: an emitted
// artifact, a template reference, or a writes_adr directive is a workflow-
// DECLARED input for this phase, not feedback from a prior phase's execution,
// so a fresh-context reviewer still receives them.
//   - emits: reads prior phases' declared output files and injects their content
//     (asset-runtime-gap §1.3). Missing files are silently skipped.
//   - writes_adr: tells the agent where and how to write an ADR (design-stage)
//   - uses_template: reads THIS phase's primary AI-SDLC template file (dimension-
//     specific guidance, e.g. STRIDE for security-review). Missing templates WARN
//     via stderr but never block.
//   - secondary_template: reads THIS phase's OPTIONAL second AI-SDLC template file
//     (review.yml's performance-reliability-review pairs 05-performance-review.md
//     with 06-production-readiness.md — one phase, two review dimensions). Same
//     missing-file WARN-not-block behavior as uses_template. Empty when the phase
//     declares no secondary_template (the default — byte-for-byte unchanged output).
func appendArtifactContext(ctx []string, repoRoot string, emitsFiles []string, templatePath, secondaryTemplatePath string, wa *asset.WritesADR) []string {
	if ec := emitsContext(repoRoot, emitsFiles, func(msg string) { fmt.Fprintln(os.Stderr, msg) }); len(ec) > 0 {
		ctx = append(ctx, ec...)
	}
	if wac := writesAdrContext(repoRoot, wa); wac != "" {
		ctx = append(ctx, wac)
	}
	if tc := usesTemplateContext(repoRoot, templatePath); tc != "" {
		ctx = append(ctx, tc)
	}
	if stc := secondaryTemplateContext(repoRoot, secondaryTemplatePath); stc != "" {
		ctx = append(ctx, stc)
	}
	return ctx
}
