// approve.go — `forge approve` and `forge reject` CLI subcommands: manage
// human-gate approval/rejection markers in .forge/<stage>.approved and
// .forge/<stage>.rejected.
package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/statefs"
)

// approvalStages is deliberately closed over the shipped workflow spine.
// Marker names are security-sensitive path components, not free-form labels:
// accepting an arbitrary value here would let "../x" escape .forge.
var approvalStages = []string{"discover", "design", "review", "build", "deploy", "rollback", "evolve"}

const decisionMarkerFormat = "forgeos.approval.v2"

type decisionMarker struct {
	Format         string `json:"_format"`
	Stage          string `json:"stage"`
	Decision       string `json:"decision"`
	ActorHint      string `json:"actor_hint"`
	CreatedAt      string `json:"created_at"`
	SourceRevision string `json:"source_revision,omitempty"`
	ArtifactDigest string `json:"artifact_digest,omitempty"`
}

// cmdApprove implements `forge approve <stage> [--root DIR]` and
// `forge approve list [--root DIR]`. Approving a stage creates a
// .forge/<stage>.approved marker file that the human_gate stop reads.
func cmdApprove(args []string) int {
	stage, root, err := parseApprovalArgs("approve", args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approve: %v\n", err)
		fmt.Fprintln(os.Stderr, "usage: forge approve <stage>|list [--root DIR]")
		return 2
	}
	root = gate.RepoRoot(root)
	switch stage {
	case "list":
		return cmdApproveList(root)
	default:
		return writeApproval(root, stage, true)
	}
}

// cmdReject implements `forge reject <stage> [--root DIR]`. Rejecting a stage
// creates a .forge/<stage>.rejected marker file that triggers on_rejected
// loop-back on `forge run`, remains after failed rework, and is consumed only
// after successful rework.
func cmdReject(args []string) int {
	stage, root, err := parseApprovalArgs("reject", args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge reject: %v\n", err)
		fmt.Fprintln(os.Stderr, "usage: forge reject <stage> [--root DIR]")
		return 2
	}
	root = gate.RepoRoot(root)
	return writeApproval(root, stage, false)
}

// parseApprovalArgs accepts --root before OR after the single positional
// argument. flag.FlagSet stops parsing at the first positional argument, which
// contradicted the documented `approve <stage> [--root DIR]` form.
func parseApprovalArgs(command string, args []string) (stage, root string, err error) {
	var positional []string
	rootSeen := false
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "--root" || arg == "-root":
			if rootSeen {
				return "", "", fmt.Errorf("--root may be supplied only once")
			}
			if i+1 >= len(args) {
				return "", "", fmt.Errorf("--root requires a directory")
			}
			rootSeen = true
			root = args[i+1]
			if root == "" {
				return "", "", fmt.Errorf("--root requires a directory")
			}
			i++
		case strings.HasPrefix(arg, "--root=") || strings.HasPrefix(arg, "-root="):
			if rootSeen {
				return "", "", fmt.Errorf("--root may be supplied only once")
			}
			rootSeen = true
			root = strings.SplitN(arg, "=", 2)[1]
			if root == "" {
				return "", "", fmt.Errorf("--root requires a directory")
			}
		case strings.HasPrefix(arg, "-"):
			return "", "", fmt.Errorf("unknown option %q", arg)
		default:
			positional = append(positional, arg)
		}
	}
	if len(positional) != 1 || positional[0] == "" {
		return "", "", fmt.Errorf("%s requires exactly one stage", command)
	}
	return positional[0], root, nil
}

func validateApprovalStage(stage string) error {
	for _, known := range approvalStages {
		if stage == known {
			return nil
		}
	}
	return fmt.Errorf("unknown stage %q (known: %s)", stage, strings.Join(approvalStages, ", "))
}

// approvalMarkerPath validates both the stage component and the final
// containment under .forge. The containment check is defense in depth around
// the closed stage vocabulary and must remain if the vocabulary ever becomes
// data-driven.
func approvalMarkerPath(root, stage, suffix string) (string, error) {
	if err := validateApprovalStage(stage); err != nil {
		return "", err
	}
	if suffix != ".approved" && suffix != ".rejected" {
		return "", fmt.Errorf("invalid approval marker suffix %q", suffix)
	}
	dotForge := forgeDir(root)
	marker := filepath.Join(dotForge, stage+suffix)
	rel, err := filepath.Rel(dotForge, marker)
	if err != nil || filepath.IsAbs(rel) || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("marker for stage %q escapes .forge", stage)
	}
	if _, _, err := containedRepoPath(root, filepath.Join(".forge", stage+suffix)); err != nil {
		return "", fmt.Errorf("marker for stage %q escapes repo: %w", stage, err)
	}
	return marker, nil
}

// writeApproval records one of two mutually-exclusive decisions:
//   - approved is durable while its bound context remains valid (delivery stages)
//     and until a later reject supersedes it;
//   - rejected is retained across failed rework and consumed only after an
//     actionable on_rejected.loop_back completes successfully.
//
// The desired marker is prepared before the opposite marker is removed, then
// atomically renamed into place. A failed preparation therefore preserves the
// prior decision, while a successful transition never leaves both decisions.
func writeApproval(root, stage string, approve bool) int {
	markerSuffix, oppositeSuffix, action := approvalDecisionShape(approve)
	markerPath, err := approvalMarkerPath(root, stage, markerSuffix)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: %v\n", action, err)
		return 2
	}
	oppositePath, err := approvalMarkerPath(root, stage, oppositeSuffix)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: %v\n", action, err)
		return 2
	}
	dotForge := forgeDir(root)
	if err := rejectTrackedForgeControlState(root); err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: %v\n", action, err)
		return 1
	}
	if err := statefs.EnsurePrivateDir(dotForge); err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: cannot secure .forge directory: %v\n", action, err)
		return 1
	}
	marker := decisionMarker{
		Format: decisionMarkerFormat, Stage: stage, Decision: action,
		ActorHint: approvalActorHint(), CreatedAt: time.Now().UTC().Format(time.RFC3339),
	}
	if err := bindReleaseDecision(root, stage, approve, &marker); err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: cannot bind delivery approval: %v\n", action, err)
		return 1
	}
	if err := installDecisionMarker(dotForge, markerPath, oppositePath, marker); err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: %v\n", action, err)
		return 1
	}
	if err := finishReleaseDecision(root, stage, approve); err != nil {
		fmt.Fprintf(os.Stderr, "forge %s: decision recorded but validation receipt invalidation failed: %v\n", action, err)
		return 1
	}
	narrateApprovalDecision(stage, markerPath, approve, action)
	return 0
}

func approvalDecisionShape(approve bool) (markerSuffix, oppositeSuffix, action string) {
	if approve {
		return ".approved", ".rejected", "approved"
	}
	return ".rejected", ".approved", "rejected"
}

func bindReleaseDecision(root, stage string, approve bool, marker *decisionMarker) error {
	if !approve || !releaseApprovalStage(stage) {
		return nil
	}
	if err := verifyReleaseValidationReceipt(root, stage); err != nil {
		return err
	}
	context, err := currentReleaseApprovalContext(root, stage)
	if err != nil {
		return err
	}
	marker.SourceRevision = context.SourceRevision
	marker.ArtifactDigest = context.ArtifactDigest
	return nil
}

func finishReleaseDecision(root, stage string, approve bool) error {
	if approve || !releaseApprovalStage(stage) {
		return nil
	}
	return invalidateReleaseValidationReceipt(root, stage)
}

func narrateApprovalDecision(stage, markerPath string, approve bool, action string) {
	fmt.Printf("forge %s: stage %s (%s)\n", action, stage, markerPath)
	fmt.Println("  Next: forge run <next-workflow> --chain (or manually)")
	if approve {
		durability := "persists until a later forge reject supersedes it"
		if releaseApprovalStage(stage) {
			durability = "is bound to the current source revision and release-artifact digest"
		}
		fmt.Println("  Approval " + durability)
	} else {
		fmt.Println("  Rejection is retained after failed rework and consumed after an actionable on_rejected loop-back succeeds")
	}
}

func installDecisionMarker(dotForge, markerPath, oppositePath string, marker decisionMarker) error {
	if err := statefs.EnsurePrivateDir(dotForge); err != nil {
		return fmt.Errorf("cannot secure .forge directory: %w", err)
	}
	for _, path := range []string{markerPath, oppositePath} {
		if _, _, err := statefs.InspectRegular(path); err != nil {
			return fmt.Errorf("cannot secure marker %s: %w", path, err)
		}
	}
	data, err := json.Marshal(marker)
	if err != nil {
		return fmt.Errorf("cannot encode marker %s: %w", markerPath, err)
	}
	if err := statefs.AtomicWrite(markerPath, append(data, '\n'), 0o600); err != nil {
		return fmt.Errorf("cannot install marker %s: %w", markerPath, err)
	}
	if err := statefs.RemoveRegular(oppositePath); err != nil {
		return fmt.Errorf("new marker installed but cannot supersede opposite marker %s: %w", oppositePath, err)
	}
	return nil
}

func approvalActorHint() string {
	for _, name := range []string{"USER", "LOGNAME"} {
		if value := strings.TrimSpace(os.Getenv(name)); value != "" {
			return value
		}
	}
	return "unknown-local-operator"
}

func releaseApprovalStage(stage string) bool {
	return stage == "deploy" || stage == "rollback"
}

// validReleaseApproval rejects the generic one-shot --approved shortcut and
// legacy/empty markers for production-changing stages. The local actor hint is
// audit metadata, not cryptographic identity; external evidence remains human
// responsibility at this trust boundary.
func validReleaseApproval(root, stage string) bool {
	if !validReleaseValidationReceipt(root, stage) {
		return false
	}
	if !releaseRejectionMarkerAbsent(root, stage) {
		return false
	}
	state, err := approvalDecisionState(root, stage)
	if err != nil || state != "approved (persistent until superseded)" {
		return false
	}
	data, err := readPrivateSingleLinkFile(approvalPath(root, stage), 64<<10)
	if err != nil {
		return false
	}
	var marker decisionMarker
	if json.Unmarshal(data, &marker) != nil {
		return false
	}
	if marker.Format != decisionMarkerFormat || marker.Stage != stage ||
		marker.Decision != "approved" || marker.CreatedAt == "" {
		return false
	}
	current, err := currentReleaseApprovalContext(root, stage)
	if err != nil {
		return false
	}
	if marker.SourceRevision != current.SourceRevision ||
		marker.ArtifactDigest != current.ArtifactDigest {
		return false
	}
	state, err = approvalDecisionState(root, stage)
	return err == nil && state == "approved (persistent until superseded)" &&
		releaseRejectionMarkerAbsent(root, stage)
}

func releaseRejectionMarkerAbsent(root, stage string) bool {
	_, present, err := statefs.InspectRegular(rejectionPath(root, stage))
	return err == nil && !present
}

type approvalDecision struct {
	stage, state string
}

// cmdApproveList reports recorded decisions, not "pending approvals": an
// .approved marker means approval has already been granted, while a .rejected
// marker is pending successful rework. Legacy conflicting markers are
// surfaced as an error instead of silently choosing one.
func cmdApproveList(root string) int {
	if err := rejectTrackedForgeControlState(root); err != nil {
		fmt.Fprintf(os.Stderr, "forge approve list: %v\n", err)
		return 1
	}
	dotForge := forgeDir(root)
	_, present, err := statefs.InspectDir(dotForge)
	if err == nil && !present {
		fmt.Println("forge approve: no recorded approval decisions")
		return 0
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approve list: cannot inspect %s: %v\n", dotForge, err)
		return 1
	}
	cp, checkpointFound, err := persist.Load(filepath.Join(dotForge, "checkpoint.json"))
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approve list: cannot read checkpoint: %v\n", err)
		return 1
	}
	cpInfo := ""
	if checkpointFound {
		cpInfo = fmt.Sprintf(" (checkpoint: iteration=%d, roadmap=%.0f%%)", cp.Iteration, cp.RoadmapCompletion*100)
	}
	decisions, conflict, err := collectApprovalDecisions(root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approve list: %v\n", err)
		return 1
	}
	if len(decisions) == 0 {
		fmt.Println("forge approve: no recorded approval decisions")
		return 0
	}
	fmt.Println("Approval decisions:")
	for _, d := range decisions {
		fmt.Printf("  %s: %s%s\n", d.stage, d.state, cpInfo)
	}
	if conflict {
		fmt.Println("  Resolve conflicts with forge approve <stage> or forge reject <stage>")
		return 1
	}
	return 0
}

func collectApprovalDecisions(root string) ([]approvalDecision, bool, error) {
	var decisions []approvalDecision
	conflict := false
	for _, stage := range approvalStages {
		state, err := approvalDecisionState(root, stage)
		if err != nil {
			return nil, false, err
		}
		if state != "" {
			decisions = append(decisions, approvalDecision{stage, state})
		}
		if strings.HasPrefix(state, "CONFLICT") {
			conflict = true
		}
	}
	return decisions, conflict, nil
}

func approvalDecisionState(root, stage string) (string, error) {
	approvedPath, err := approvalMarkerPath(root, stage, ".approved")
	if err != nil {
		return "", err
	}
	rejectedPath, err := approvalMarkerPath(root, stage, ".rejected")
	if err != nil {
		return "", err
	}
	approved, err := markerExists(approvedPath)
	if err != nil {
		return "", fmt.Errorf("cannot inspect %s: %w", approvedPath, err)
	}
	rejected, err := markerExists(rejectedPath)
	if err != nil {
		return "", fmt.Errorf("cannot inspect %s: %w", rejectedPath, err)
	}
	switch {
	case approved && rejected:
		return "CONFLICT (approved + rejected)", nil
	case approved:
		return "approved (persistent until superseded)", nil
	case rejected:
		return "rejected (pending successful rework)", nil
	default:
		return "", nil
	}
}

func markerExists(path string) (bool, error) {
	_, present, err := statefs.InspectRegular(path)
	return present, err
}

type releaseApprovalContext struct {
	SourceRevision string
	ArtifactDigest string
}

var releaseApprovalFiles = map[string][]string{
	"deploy": {
		"docs/release/release-manifest.yml",
		"docs/release/deployment-plan.md",
		"docs/release/deployment-runbook.md",
		"docs/release/go-no-go-checklist.md",
		"docs/release/deployment-validation.md",
	},
	"rollback": {
		"docs/release/rollback-plan.md",
		"docs/release/rollback-runbook.md",
		"docs/release/rollback-checklist.md",
		"docs/release/rollback-validation.md",
	},
}

var releaseApprovalContextFiles = map[string][]string{
	"deploy": releaseApprovalFiles["deploy"],
	"rollback": append(
		append([]string(nil), releaseApprovalFiles["deploy"][0]),
		releaseApprovalFiles["rollback"]...,
	),
}

func currentReleaseApprovalContext(root, stage string) (releaseApprovalContext, error) {
	files, ok := releaseApprovalContextFiles[stage]
	if !ok {
		return releaseApprovalContext{}, fmt.Errorf("stage %q is not a delivery stage", stage)
	}
	revision, err := sourceStateRevision(root)
	if err != nil {
		return releaseApprovalContext{}, err
	}
	digest, err := digestReleaseFiles(root, files)
	if err != nil {
		return releaseApprovalContext{}, err
	}
	return releaseApprovalContext{SourceRevision: revision, ArtifactDigest: digest}, nil
}

func digestReleaseFiles(root string, files []string) (string, error) {
	hash := sha256.New()
	for _, path := range files {
		_, relative, err := containedRepoPath(root, path)
		if err != nil {
			return "", fmt.Errorf("release artifact %q: %w", path, err)
		}
		data, present, err := readReleaseFileBytes(root, path)
		if err != nil {
			return "", fmt.Errorf("release artifact %q is not verifiable: %w", path, err)
		}
		if !present {
			return "", fmt.Errorf("release artifact %q is missing", path)
		}
		if len(bytes.TrimSpace(data)) == 0 {
			return "", fmt.Errorf("release artifact %q is empty", path)
		}
		writeDigestPart(hash, filepath.ToSlash(relative), data)
	}
	return fmt.Sprintf("sha256:%x", hash.Sum(nil)), nil
}

func writeDigestPart(dst io.Writer, name string, data []byte) {
	_, _ = fmt.Fprintf(dst, "%d:%s:%d:", len(name), name, len(data))
	_, _ = dst.Write(data)
}
