package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/statefs"
)

const releaseValidationReceiptFormat = "forgeos.release-validation.v1"

type releaseValidationReceipt struct {
	Format                string `json:"_format"`
	Stage                 string `json:"stage"`
	Phase                 string `json:"phase"`
	RunID                 string `json:"run_id"`
	Model                 string `json:"model"`
	AgentExecutableSHA256 string `json:"agent_executable_sha256"`
	PromptSHA256          string `json:"prompt_sha256"`
	SourceRevision        string `json:"source_revision"`
	ArtifactDigest        string `json:"artifact_digest"`
	Verdict               string `json:"verdict"`
	CreatedAt             string `json:"created_at"`
}

func releaseValidationReceiptPath(root, stage string) (string, error) {
	if !releaseApprovalStage(stage) {
		return "", fmt.Errorf("stage %q is not a delivery stage", stage)
	}
	path, relative, err := containedRepoPath(root, filepath.Join(".forge", stage+".validation.json"))
	if err != nil {
		return "", err
	}
	if filepath.ToSlash(relative) != ".forge/"+stage+".validation.json" {
		return "", fmt.Errorf("invalid release validation receipt path %q", relative)
	}
	return path, nil
}

func invalidateReleaseValidationReceipt(root, stage string) error {
	path, err := releaseValidationReceiptPath(root, stage)
	if err != nil {
		return err
	}
	if err := statefs.RemoveRegular(path); err != nil {
		return fmt.Errorf("remove stale release validation receipt: %w", err)
	}
	return nil
}

func writeReleaseValidationReceipt(root, stage string, phase assetPhaseReceipt, context releaseApprovalContext) error {
	path, err := releaseValidationReceiptPath(root, stage)
	if err != nil {
		return err
	}
	receipt := releaseValidationReceipt{
		Format: releaseValidationReceiptFormat, Stage: stage, Phase: phase.Name,
		RunID: phase.RunID, Model: phase.Model,
		AgentExecutableSHA256: strings.ToLower(strings.TrimSpace(phase.AgentSHA256)),
		PromptSHA256:          phase.PromptSHA256,
		SourceRevision:        context.SourceRevision,
		ArtifactDigest:        context.ArtifactDigest,
		Verdict:               VerdictApprove,
		CreatedAt:             time.Now().UTC().Format(time.RFC3339Nano),
	}
	if err := validateReleaseValidationReceiptFields(receipt); err != nil {
		return err
	}
	return installReleaseValidationReceipt(root, path, receipt)
}

type assetPhaseReceipt struct {
	Name         string
	RunID        string
	Model        string
	AgentSHA256  string
	PromptSHA256 string
}

func installReleaseValidationReceipt(root, path string, receipt releaseValidationReceipt) error {
	dotForge := forgeDir(root)
	if err := statefs.EnsurePrivateDir(dotForge); err != nil {
		return fmt.Errorf("secure receipt directory: %w", err)
	}
	data, err := json.Marshal(receipt)
	if err != nil {
		return err
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		return fmt.Errorf("install validation receipt: %w", err)
	}
	return nil
}

func verifyReleaseValidationReceipt(root, stage string) error {
	path, err := releaseValidationReceiptPath(root, stage)
	if err != nil {
		return err
	}
	data, err := readPrivateSingleLinkFile(path, 64<<10)
	if err != nil {
		return fmt.Errorf("read release validation receipt: %w", err)
	}
	var receipt releaseValidationReceipt
	if err := json.Unmarshal(data, &receipt); err != nil {
		return fmt.Errorf("decode release validation receipt: %w", err)
	}
	if err := validateReleaseValidationReceiptFields(receipt); err != nil {
		return err
	}
	if receipt.Stage != stage || receipt.Phase != releaseValidationPhaseName(stage) {
		return fmt.Errorf("release validation receipt stage/phase mismatch")
	}
	current, err := currentReleaseApprovalContext(root, stage)
	if err != nil {
		return err
	}
	if receipt.SourceRevision != current.SourceRevision ||
		receipt.ArtifactDigest != current.ArtifactDigest {
		return fmt.Errorf("release validation receipt is stale")
	}
	report := releaseApprovalFiles[stage][len(releaseApprovalFiles[stage])-1]
	verdict, err := releaseArtifactVerdict(root, report)
	if err != nil || verdict != VerdictApprove {
		return fmt.Errorf("release validation report does not end in APPROVE")
	}
	return nil
}

func validReleaseValidationReceipt(root, stage string) bool {
	return verifyReleaseValidationReceipt(root, stage) == nil
}

func validateReleaseValidationReceiptFields(receipt releaseValidationReceipt) error {
	if receipt.Format != releaseValidationReceiptFormat || receipt.Stage == "" ||
		receipt.Phase == "" || receipt.RunID == "" || receipt.Model == "" ||
		receipt.SourceRevision == "" || receipt.ArtifactDigest == "" ||
		receipt.Verdict != VerdictApprove || receipt.CreatedAt == "" {
		return fmt.Errorf("release validation receipt is incomplete or unsupported")
	}
	if !validReleaseSHA256(receipt.AgentExecutableSHA256) ||
		!validReleaseSHA256(receipt.PromptSHA256) {
		return fmt.Errorf("release validation receipt hashes are invalid")
	}
	if _, err := time.Parse(time.RFC3339Nano, receipt.CreatedAt); err != nil {
		return fmt.Errorf("release validation receipt timestamp is invalid")
	}
	return nil
}

func validReleaseSHA256(value string) bool {
	if len(value) != sha256.Size*2 || value != strings.ToLower(value) {
		return false
	}
	decoded, err := hex.DecodeString(value)
	return err == nil && len(decoded) == sha256.Size
}

func parsePinnedReleaseArgs(args []string) (string, string, []string, error) {
	if len(args) < 3 || args[2] != "--" {
		return "", "", nil, fmt.Errorf("invalid internal launcher arguments")
	}
	path := filepath.Clean(args[0])
	if !filepath.IsAbs(path) {
		return "", "", nil, fmt.Errorf("pinned executable must be an absolute canonical path")
	}
	expected := strings.ToLower(strings.TrimSpace(args[1]))
	if !validReleaseSHA256(expected) {
		return "", "", nil, fmt.Errorf("pinned executable SHA-256 is invalid")
	}
	return path, expected, append([]string(nil), args[3:]...), nil
}

func releaseValidationPhaseName(stage string) string {
	if stage == "deploy" {
		return "release-plan-validation"
	}
	if stage == "rollback" {
		return "rollback-plan-validation"
	}
	return ""
}

func releaseArtifactVerdict(root, relative string) (string, error) {
	data, present, err := readReleaseFileBytes(root, relative)
	if err != nil {
		return "", err
	}
	if !present {
		return "", fmt.Errorf("validation report %q is missing", relative)
	}
	verdict, ok := parseReviewerVerdict(string(data))
	if !ok {
		return "", fmt.Errorf("validation report %q has no exact final verdict", relative)
	}
	return verdict, nil
}

func readPrivateSingleLinkFile(path string, limit int64) ([]byte, error) {
	before, present, err := statefs.InspectRegular(path)
	if err != nil {
		return nil, err
	}
	if !present {
		return nil, os.ErrNotExist
	}
	if before.Mode().Perm()&0o077 != 0 {
		return nil, fmt.Errorf("file must be private, regular, and single-link")
	}
	data, found, err := statefs.ReadRegular(path, limit)
	if err != nil {
		return nil, err
	}
	if !found {
		return nil, os.ErrNotExist
	}
	return data, nil
}
