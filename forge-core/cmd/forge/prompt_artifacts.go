// prompt_artifacts.go — the workflow-DECLARED artifact-context lanes of
// buildPrompt: `emits` (prior phases' declared output files) and the paired
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
)

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
		fullPath := path
		if !filepath.IsAbs(path) {
			fullPath = filepath.Join(repoRoot, path)
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
		blocks = append(blocks, contextMarker("emit:"+filepath.Base(fullPath), content))
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
	fullPath := templatePath
	if !filepath.IsAbs(templatePath) {
		fullPath = filepath.Join(repoRoot, templatePath)
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
	return contextMarker("template:"+templatePath, content)
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

// appendArtifactContext appends the emitted-artifact, uses_template, and
// secondary_template context lanes onto ctx. Unlike appendFeedbackLanes, these are
// NEVER gated on FreshContext: an emitted artifact or a template reference is a
// workflow-DECLARED input for this phase, not feedback from a prior phase's
// execution, so a fresh-context reviewer still receives them.
//   - emits: reads prior phases' declared output files and injects their content
//     (asset-runtime-gap §1.3). Missing files are silently skipped.
//   - uses_template: reads THIS phase's primary AI-SDLC template file (dimension-
//     specific guidance, e.g. STRIDE for security-review). Missing templates WARN
//     via stderr but never block.
//   - secondary_template: reads THIS phase's OPTIONAL second AI-SDLC template file
//     (review.yml's performance-reliability-review pairs 05-performance-review.md
//     with 06-production-readiness.md — one phase, two review dimensions). Same
//     missing-file WARN-not-block behavior as uses_template. Empty when the phase
//     declares no secondary_template (the default — byte-for-byte unchanged output).
func appendArtifactContext(ctx []string, repoRoot string, emitsFiles []string, templatePath, secondaryTemplatePath string) []string {
	if ec := emitsContext(repoRoot, emitsFiles, func(msg string) { fmt.Fprintln(os.Stderr, msg) }); len(ec) > 0 {
		ctx = append(ctx, ec...)
	}
	if tc := usesTemplateContext(repoRoot, templatePath); tc != "" {
		ctx = append(ctx, tc)
	}
	if stc := secondaryTemplateContext(repoRoot, secondaryTemplatePath); stc != "" {
		ctx = append(ctx, stc)
	}
	return ctx
}
