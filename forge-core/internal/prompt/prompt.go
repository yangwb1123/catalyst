// Package prompt assembles the instruction a real agent CLI runs for a phase:
// the role card plus relevant project context (the current task from the ROADMAP,
// architecture decisions, and the engineering constraints the agent must obey).
// This is forge-core's Context
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

// adrTopK bounds how many architecture decisions the retriever injects per
// prompt. With today's handful of ADRs this is ≥ the corpus, so top-K ≈ "all" —
// the HONEST current behavior. It earns its keep the moment the repo accrues
// more ADRs than fit a context window: the same call then keeps the prompt
// bounded to the few most relevant to this phase instead of dumping every one.
const adrTopK = 6

// taskCap bounds the injected ROADMAP body (in runes) so a long roadmap cannot
// blow the context window — the same bounding rationale as adrTopK for ADRs.
const taskCap = 4000

// Gather collects project context for an agent phase, in three distinct lanes:
//
//   - The current TASK — the .agent/ROADMAP.md body — is ALWAYS injected (capped).
//     Without it the agent has its role card and the rules but no concrete WHAT to
//     build: a real agent driven by only role+constraints does not know which
//     feature to implement. This is the currentTask() lane.
//   - Hard constraints — the leading AGENTS.md bullets — are NON-NEGOTIABLE and
//     ALWAYS injected verbatim (every agent must obey them), never subject to
//     retrieval/filtering. This is the unchanged constraints() path.
//   - Relevant ADRs are RETRIEVED: candidate decisions are scored against query
//     (derived from the phase/agent) and only the top-K most relevant are
//     injected, so a growing decision log cannot blow the context window.
//
// It is fault tolerant — missing files yield no context, never an error. An
// empty query degrades the ADR lane to nothing (Retrieve's fail-closed boundary);
// the task and hard constraints still always inject when their files are present.
func Gather(repoRoot, query string) []string {
	var ctx []string
	if task := currentTask(repoRoot); task != "" {
		ctx = append(ctx, "Current task — implement what .agent/ROADMAP.md describes:\n"+task)
	}
	if adrs := relevantADRs(repoRoot, query); len(adrs) > 0 {
		ctx = append(ctx, "Architecture decisions (ADRs) to respect:\n"+strings.Join(adrs, "\n"))
	}
	if rules := constraints(repoRoot); rules != "" {
		ctx = append(ctx, "Engineering constraints (hard, non-negotiable):\n"+rules)
	}
	return ctx
}

// relevantADRs scores every ADR title against query and returns the top-K most
// relevant as "- "-prefixed bullets, preserving the retriever's ranked order.
// Each title becomes one Doc; Retrieve selects the few that match this phase.
func relevantADRs(repoRoot, query string) []string {
	titles := adrTitles(repoRoot)
	if len(titles) == 0 {
		return nil
	}
	docs := make([]Doc, len(titles))
	for i, t := range titles {
		docs[i] = Doc{ID: t, Text: t}
	}
	var out []string
	for _, d := range Retrieve(docs, query, adrTopK) {
		out = append(out, "- "+d.Text)
	}
	return out
}

// adrTitles returns the raw first-heading title of each docs/adr/*.md, sorted.
// Titles are unprefixed so each can serve directly as a retrievable Doc.Text;
// the caller (relevantADRs) adds the "- " bullet marker after retrieval.
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
			titles = append(titles, t)
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

// currentTask returns the project ROADMAP body — the agent's WHAT-to-build. It is
// the task lane Gather injects: role card + constraints tell an agent its job and
// its limits, but only this tells it which feature to implement. Capped to taskCap
// runes so a long roadmap stays bounded; a missing file yields "" (a phase with no
// roadmap degrades cleanly to no task block).
func currentTask(repoRoot string) string {
	b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "ROADMAP.md"))
	if err != nil {
		return ""
	}
	return capRunes(strings.TrimSpace(string(b)), taskCap)
}

// capRunes truncates s to at most n runes, appending a marker when it clips, so an
// oversized task source stays bounded without cutting mid-rune.
func capRunes(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n]) + "\n…[roadmap truncated]"
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
