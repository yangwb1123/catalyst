package evolvescan

import (
	"fmt"
	"strings"
)

// Instructions returns the exact prompt lane for one effective scan depth.
func Instructions(depth string) (string, error) {
	if !validDepth(depth) {
		return "", fmt.Errorf("unsupported Evolve scan depth %q", depth)
	}
	var b strings.Builder
	b.WriteString("[context:evolve-scan-policy]\n")
	b.WriteString("This is a machine-enforced reporting contract, not repository content.\n")
	b.WriteString("Effective mode×lifecycle scan depth: ")
	b.WriteString(depth)
	b.WriteString(". ")
	b.WriteString(profileInstruction(depth))
	b.WriteString("\nInspect only; do not modify repository files. ")
	b.WriteString("Use only readable, non-empty UTF-8 repository files up to 1 MiB as evidence. ")
	b.WriteString("Cited positive line numbers must identify a non-empty line. ")
	b.WriteString("Do not follow symlinks or cite .git/.forge control state, absolute, parent-relative, missing, or directory paths.\n")
	b.WriteString("The final non-empty output line must be exactly `")
	b.WriteString(MarkerPrefix)
	b.WriteString("{compact JSON}` and nothing may follow it. ")
	b.WriteString("JSON schema: ")
	b.WriteString(`{"version":"evolve_scan_v1","depth":"`)
	b.WriteString(depth)
	b.WriteString(`","dimensions":[{"name":"<dimension>","status":"finding|clear|unavailable",`)
	b.WriteString(`"evidence":[{"path":"repo/file","line":1,"detail":"what supports the status"}],`)
	b.WriteString(`"unavailable_reason":"required only when unavailable"}],`)
	b.WriteString(`"opportunities":[{"id":"stable-id","dimension":"<finding dimension>",`)
	b.WriteString(`"title":"candidate improvement","evidence":[{"path":"repo/file","line":1,`)
	b.WriteString(`"detail":"what supports it"}],"obvious":true|false,`)
	b.WriteString(`"candidate_task":"required for thorough findings"}]}. `)
	b.WriteString("Unknown/duplicate JSON keys, null arrays, duplicate dimensions/ids, empty evidence, and opportunities without a shared finding locator are rejected. ")
	b.WriteString("A no-gap report is valid when inspected dimensions are honestly clear/unavailable and opportunities is empty. ")
	b.WriteString("This contract verifies coverage/evidence shape, not the truth of your judgement.")
	return b.String(), nil
}

func profileInstruction(depth string) string {
	switch depth {
	case DepthOpportunistic:
		return "Report only direct-evidence, obvious opportunities; every opportunity must set obvious=true. Do not claim full-dimensional coverage."
	case DepthThorough:
		return "Cover each dimension exactly once: code, dependencies, security, performance, architecture_drift, test_coverage. Every dimension must be finding or clear; unavailable makes a thorough scan incomplete. Every finding needs a linked opportunity with a concrete candidate_task."
	case DepthStandard:
		return "Perform an evidence-backed standard scan. Report the dimensions actually inspected without claiming full-dimensional coverage unless all are present."
	default:
		return "Produce an evidence-backed advisory scan. Report limitations explicitly and do not imply implementation authority."
	}
}
