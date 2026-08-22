package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"path/filepath"
	"time"

	"forgeos/forge-core/internal/materiality"
	"forgeos/forge-core/internal/statefs"
)

const (
	chainStateFormatV1 = "forgeos.chain-state.v1"
	chainStateFormatV2 = "forgeos.chain-state.v2"
	chainStateFormatV3 = "forgeos.chain-state.v3"
	chainStateFormatV4 = "forgeos.chain-state.v4"
	chainStateFormat   = "forgeos.chain-state.v5"
)

// chainState is the versioned durable chain cursor and diagnostic snapshot.
type chainState struct {
	Format           string            `json:"_format"`
	RunID            string            `json:"run_id,omitempty"`
	Status           string            `json:"status"`
	EntryStage       string            `json:"entry_stage,omitempty"`
	CurrentStage     string            `json:"current_stage,omitempty"`
	CompletedStages  []string          `json:"completed_stages,omitempty"`
	Mode             string            `json:"mode,omitempty"`
	Lifecycle        string            `json:"lifecycle,omitempty"`
	Materiality      string            `json:"materiality"`
	WorkflowDigests  map[string]string `json:"workflow_digests"`
	BoundStages      []string          `json:"bound_workflow_stages"`
	ReceiptHead      string            `json:"agent_output_receipt_head_sha256"`
	PhaseReceipts    map[string]string `json:"phase_output_receipts"`
	StageReceipts    map[string]string `json:"stage_output_receipts"`
	ApprovalContexts map[string]string `json:"stage_approval_contexts"`
	InheritedStages  []string          `json:"inherited_stages,omitempty"`
	Reason           string            `json:"reason,omitempty"`
	AgentCalls       int               `json:"agent_calls,omitempty"`
	MaxAgentCalls    int               `json:"max_agent_calls,omitempty"`
	MaxChainStages   int               `json:"max_chain_stages"`
	SpentUsdMicros   int64             `json:"spent_usd_micros,omitempty"`
	BudgetCapMicros  int64             `json:"budget_cap_micros,omitempty"`
	UpdatedAtUnix    int64             `json:"updated_at_unix"`
}

func chainStatePath(root string) string {
	return filepath.Join(forgeDir(root), "chain-state.json")
}

func saveChainState(root string, state chainState) error {
	bound, err := materiality.Normalize(state.Materiality)
	if err != nil {
		return fmt.Errorf("encode chain state materiality: %w", err)
	}
	state.Format, state.Materiality = chainStateFormat, bound
	if state.WorkflowDigests == nil {
		state.WorkflowDigests = map[string]string{}
	}
	state.ensureRecoveryMaps()
	if err := validateChainWorkflowDigestMap(state.WorkflowDigests); err != nil {
		return fmt.Errorf("encode chain state: %w", err)
	}
	state.UpdatedAtUnix = time.Now().Unix()
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return fmt.Errorf("encode chain state: %w", err)
	}
	path := chainStatePath(root)
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		return fmt.Errorf("secure chain state directory: %w", err)
	}
	if err := statefs.RemoveRegular(path + ".tmp"); err != nil {
		return fmt.Errorf("reject legacy chain state temp: %w", err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		return fmt.Errorf("commit chain state: %w", err)
	}
	return nil
}

// persistChainState is the runtime persistence seam. Tests replace it to model
// an interrupted atomic commit while direct state fixtures keep using saveChainState.
var persistChainState = saveChainState

func loadChainState(root string) (chainState, bool, error) {
	data, found, err := statefs.ReadRegular(chainStatePath(root), 1<<20)
	if err != nil {
		return chainState{}, false, fmt.Errorf("read chain state: %w", err)
	}
	if !found {
		return chainState{}, false, nil
	}
	var state chainState
	if err := json.Unmarshal(data, &state); err != nil {
		return chainState{}, true, fmt.Errorf("decode chain state: %w", err)
	}
	switch state.Format {
	case chainStateFormatV1, chainStateFormatV2, chainStateFormatV3, chainStateFormatV4:
		// Historical cursors remain inspectable by status/doctor. Resume rejects
		// them because they lack all current recovery identity bindings.
		return state, true, nil
	case chainStateFormat:
		if err := validateCurrentChainStateEncoding(data, state); err != nil {
			return chainState{}, true, err
		}
		return state, true, nil
	default:
		return state, true, fmt.Errorf("unsupported chain state format %q (want %q)",
			state.Format, chainStateFormat)
	}
}

func validateCurrentChainStateEncoding(data []byte, state chainState) error {
	if err := rejectDuplicateJSONObjectKeys(data, "chain state"); err != nil {
		return err
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		return fmt.Errorf("decode chain state fields: %w", err)
	}
	for _, name := range []string{
		"materiality", "workflow_digests", "agent_output_receipt_head_sha256",
		"bound_workflow_stages",
		"phase_output_receipts", "stage_output_receipts", "stage_approval_contexts",
	} {
		raw, ok := fields[name]
		if !ok {
			return fmt.Errorf("chain state %s missing required field %q", chainStateFormat, name)
		}
		if bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
			return fmt.Errorf("chain state %s required field %q is null", chainStateFormat, name)
		}
	}
	if !materiality.Valid(state.Materiality) {
		return fmt.Errorf("chain state %s has invalid materiality %q", chainStateFormat, state.Materiality)
	}
	if err := validateBoundStageList(state); err != nil {
		return err
	}
	if err := validateSortedUniqueJSONObjectKeys(fields["workflow_digests"], "chain state workflow_digests"); err != nil {
		return err
	}
	return validateChainStateMaps(fields, state)
}

func validateResumableChainState(state chainState) error {
	if state.Format != chainStateFormat && state.Format != chainStateFormatV4 {
		return fmt.Errorf("chain state format %q is diagnostic-only; resume requires %q",
			state.Format, chainStateFormat)
	}
	switch {
	case state.RunID == "":
		return fmt.Errorf("persisted waiting chain has no run_id")
	case state.EntryStage == "" || state.CurrentStage == "":
		return fmt.Errorf("persisted waiting chain lacks entry/current stage")
	case state.Mode == "" || state.Lifecycle == "":
		return fmt.Errorf("persisted waiting chain lacks mode/lifecycle policy")
	case !materiality.Valid(state.Materiality):
		return fmt.Errorf("persisted waiting chain has invalid materiality %q", state.Materiality)
	case state.AgentCalls < 0 || state.MaxAgentCalls < 0:
		return fmt.Errorf("persisted waiting chain has negative agent-call state")
	case state.MaxChainStages < 1:
		return fmt.Errorf("persisted waiting chain has invalid max_chain_stages")
	case state.SpentUsdMicros < 0 || state.BudgetCapMicros < 0:
		return fmt.Errorf("persisted waiting chain has negative budget state")
	}
	return validateResumableChainWorkflowDigests(state)
}

func (state *chainState) ensureRecoveryMaps() {
	if state.BoundStages == nil {
		state.BoundStages = []string{}
	}
	if state.PhaseReceipts == nil {
		state.PhaseReceipts = map[string]string{}
	}
	if state.StageReceipts == nil {
		state.StageReceipts = map[string]string{}
	}
	if state.ApprovalContexts == nil {
		state.ApprovalContexts = map[string]string{}
	}
}
