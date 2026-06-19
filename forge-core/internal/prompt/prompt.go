// Package prompt assembles the instruction a real agent CLI runs for a phase:
// the role card plus relevant project context (architecture decisions and the
// engineering constraints the agent must obey). This is forge-core's Context
// Engine v1 — beyond a bare role card, the prompt now carries project ground
// truth so a real agent acts within the actual decisions and limits.
package prompt

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// Build composes the prompt: a role header, the agent's role card, and a
// project-context section. ctx may be empty (the section is then omitted).
func Build(agent, phase, mode, tier, card string, ctx []string) string {
	var b strings.Builder
	fmt.Fprintf(&b,
		"You are the %q agent in ForgeOS (phase=%s, mode=%s, tier=%s). Act strictly "+
			"within your role card and the project context below; produce this phase's output.\n\n",
		agent, phase, mode, tier)
	b.WriteString("## Role card\n")
	b.WriteString(card)
	if len(ctx) > 0 {
		b.WriteString("\n\n## Project context\n")
		b.WriteString(strings.Join(ctx, "\n\n"))
	}
	return b.String()
}

// Gather collects project context relevant to any agent phase: the recorded
// architecture decisions (ADR titles) and the hard engineering constraints. It
// is fault tolerant — missing files yield no context, never an error.
func Gather(repoRoot string) []string {
	var ctx []string
	if adrs := adrTitles(repoRoot); len(adrs) > 0 {
		ctx = append(ctx, "Architecture decisions (ADRs) to respect:\n"+strings.Join(adrs, "\n"))
	}
	if rules := constraints(repoRoot); rules != "" {
		ctx = append(ctx, "Engineering constraints (hard, non-negotiable):\n"+rules)
	}
	return ctx
}

// adrTitles returns the first-heading title of each docs/adr/*.md, sorted.
func adrTitles(repoRoot string) []string {
	dir := filepath.Join(repoRoot, "docs", "adr")
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var titles []string
	for _, e := range entries {
		if !strings.HasSuffix(e.Name(), ".md") {
			continue
		}
		if t := firstHeading(filepath.Join(dir, e.Name())); t != "" {
			titles = append(titles, "- "+t)
		}
	}
	sort.Strings(titles)
	return titles
}

// constraints returns the leading bullet lines of .agent/AGENTS.md — the hard
// engineering rules an agent must obey (capped so the prompt stays focused).
func constraints(repoRoot string) string {
	b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "AGENTS.md"))
	if err != nil {
		return ""
	}
	return leadingBullets(string(b), 6)
}

// firstHeading returns the text of a markdown file's first "# " heading.
func firstHeading(path string) string {
	b, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(b), "\n") {
		if s := strings.TrimSpace(line); strings.HasPrefix(s, "# ") {
			return strings.TrimSpace(s[2:])
		}
	}
	return ""
}

// leadingBullets returns up to max "- " bullet lines from a markdown string.
func leadingBullets(md string, max int) string {
	var out []string
	for _, line := range strings.Split(md, "\n") {
		if s := strings.TrimSpace(line); strings.HasPrefix(s, "- ") {
			out = append(out, s)
			if len(out) >= max {
				break
			}
		}
	}
	return strings.Join(out, "\n")
}
