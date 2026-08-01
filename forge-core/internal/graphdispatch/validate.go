package graphdispatch

import (
	"bytes"
	"encoding/json"
	"reflect"
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/graphplan"
)

const maxFrozenSourceBytes = 8 * 1024 * 1024
const maxProseBytes = 64 * 1024

func validateControl(snapshot ControlSnapshot) error {
	if !validControlHeader(snapshot) || validateManifestSource(snapshot.Manifest) != nil {
		return errInvalidControl
	}
	manifestSHA, err := manifestDigest(snapshot.Manifest)
	if err != nil || manifestSHA != snapshot.GraphManifestSHA256 {
		return errInvalidControl
	}
	expected, err := expectedPlan(snapshot, manifestSHA)
	if err != nil || !reflect.DeepEqual(snapshot.Plan, expected) {
		return errInvalidControl
	}
	if !reflect.DeepEqual(snapshot.Manifest.Edges, expected.Edges) ||
		!reflect.DeepEqual(snapshot.Manifest.Waves, expected.Waves) {
		return errInvalidControl
	}
	digest, err := domainDigest(snapshotDigestDomain, snapshotPayload(snapshot))
	if err != nil || digest != snapshot.SnapshotSHA256 {
		return errInvalidControl
	}
	return validateManifestSize(snapshot.Manifest)
}

func validControlHeader(snapshot ControlSnapshot) bool {
	return snapshot.V == ControlSnapshotVersion &&
		snapshot.SchedulerProtocolVersion == graphplan.SchedulerProtocolVersion &&
		snapshot.GraphRunVersion == 1 &&
		validText(snapshot.GraphRunID, 128) &&
		validText(snapshot.GraphID, 128) &&
		isLowerHexDigest(snapshot.SourceSnapshotSHA256) &&
		isLowerHexDigest(snapshot.GraphManifestSHA256) &&
		isLowerHexDigest(snapshot.CorePlanSHA256) &&
		snapshot.LastEventSeq == 1 &&
		isLowerHexDigest(snapshot.LastEventSHA256) &&
		!snapshot.ExecutionContractPresent &&
		!snapshot.DispatchAuthorityReleased &&
		snapshot.Plan.PlanSHA256 == snapshot.CorePlanSHA256 &&
		isLowerHexDigest(snapshot.SnapshotSHA256)
}

func validateManifestSource(manifest GraphManifest) error {
	source := manifest.Source
	valid := manifest.V == graphplan.GraphVersion &&
		source.GroupRunVersion == 1 && source.ContextVersion == 1 &&
		validText(source.GroupRunID, 128) && validText(source.GroupID, 128) &&
		isLowerHexDigest(source.ContextSliceSHA256) &&
		isLowerHexDigest(source.SnapshotSHA256) &&
		source.SnapshotSHA256 != "" &&
		source.SnapshotBytes >= 1 && source.SnapshotBytes <= maxFrozenSourceBytes &&
		manifest.Nodes != nil && manifest.Edges != nil && manifest.Waves != nil
	if !valid || hasNilWave(manifest.Waves) {
		return errInvalidControl
	}
	return nil
}

func expectedPlan(snapshot ControlSnapshot, manifestSHA string) (graphplan.Plan, error) {
	spec := graphplan.Spec{
		V: snapshot.Manifest.V, Manager: snapshot.Manifest.Manager,
		Nodes: snapshot.Manifest.Nodes, Edges: snapshot.Manifest.Edges,
	}
	plan, err := graphplan.Build(spec, snapshot.GraphID, manifestSHA)
	if err != nil || snapshot.SourceSnapshotSHA256 != snapshot.Manifest.Source.SnapshotSHA256 {
		return graphplan.Plan{}, errInvalidControl
	}
	return plan, nil
}

func validateManifestSize(manifest GraphManifest) error {
	encoded, err := canonicalBytes(manifestDigestView{
		Edges: manifest.Edges, Manager: manifest.Manager,
		Nodes:  manifestDigestNodes(manifest.Nodes),
		Source: manifestDigestSource(manifest.Source),
		V:      manifest.V, Waves: manifest.Waves,
	})
	if err != nil || len(encoded) == 0 || len(encoded) > graphplan.MaxSpecBytes {
		return errInvalidControl
	}
	return nil
}

func hasNilWave(waves [][]string) bool {
	for _, wave := range waves {
		if wave == nil {
			return true
		}
	}
	return false
}

func validateOptions(options ExecutionOptions) error {
	valid := validEndpoint(options.Endpoint) && validText(options.Model, MaxModelBytes) &&
		inRange(options.MaxOutputTokens, MaxOutputTokens) &&
		inRange(options.MaxModelOutputBytes, MaxModelOutputBytes) &&
		inRange(options.MaxModelEvents, MaxModelEvents) &&
		inRange(options.TimeoutMilliseconds, MaxTimeoutMilliseconds) &&
		inRange(options.MaxCostUSDMicros, MaxCostUSDMicros) &&
		isLowerHexDigest(options.PricingSnapshotSHA256) &&
		inRange(options.MaxResultBytes, MaxResultBytes)
	if !valid {
		return errInvalidControl
	}
	return nil
}

func inRange(value, maximum uint64) bool {
	return value >= 1 && value <= maximum
}

func validateContract(contract NodeExecutionContract) error {
	if !validContractHeader(contract) || !validContractNode(contract.Node) ||
		!validWorkspace(contract.Workspace) || !validProvider(contract.Provider) ||
		!validRequest(contract.Request, contract.Node.NodeID) ||
		!validBudgets(contract.Budgets) ||
		!validPolicies(contract) {
		return errInvalidControl
	}
	digest, err := domainDigest(contractDigestDomain, contractPayloadFrom(contract))
	if err != nil || digest != contract.ContractSHA256 ||
		contract.ContractID != "node-contract-"+digest {
		return errInvalidControl
	}
	return nil
}

func validContractHeader(contract NodeExecutionContract) bool {
	return contract.V == 1 &&
		contract.SchedulerProtocolVersion == graphplan.SchedulerProtocolVersion &&
		contract.NodeExecutionProtocolVersion == NodeExecutionProtocolVersion &&
		validText(contract.GraphRunID, 128) && validText(contract.GraphID, 128) &&
		isLowerHexDigest(contract.SourceSnapshotSHA256) &&
		isLowerHexDigest(contract.GraphManifestSHA256) &&
		isLowerHexDigest(contract.CorePlanSHA256) &&
		isLowerHexDigest(contract.ControlSnapshotSHA256) &&
		contract.ExpectedLastEventSeq == 1 &&
		isLowerHexDigest(contract.ExpectedLastEventSHA256) &&
		contract.ExecutionContractPresent && !contract.DispatchAuthorityReleased
}

func validContractNode(node ContractNode) bool {
	return validText(node.NodeID, 128) && node.AuthoredNodeIndex < 32 &&
		node.TopologyWaveIndex == 0 && node.Attempt == 1 &&
		validText(node.ProjectID, 128) && validText(node.MemberRole, 64) &&
		validText(node.AgentProfile, 128) && isLowerHexDigest(node.ProjectLaneSHA256) &&
		node.ProjectLaneSHA256 == rawDomainDigest(projectLaneDomain, node.ProjectID) &&
		node.SameProjectPolicy == "exclusive_until_terminal"
}

func validWorkspace(workspace WorkspacePolicy) bool {
	return workspace.Mode == "none" && workspace.RootIdentity == nil &&
		workspace.IsolationID == nil && workspace.AllowedReadPaths != nil &&
		len(workspace.AllowedReadPaths) == 0
}

func validProvider(provider ProviderPolicy) bool {
	return provider.Kind == "openai_responses" && validEndpoint(provider.Endpoint) &&
		validText(provider.Model, MaxModelBytes) && !provider.Store && provider.Stream
}

func validRequest(request NodeRequest, nodeID string) bool {
	if request.PredecessorResultReceipts == nil || request.Tools == nil ||
		len(request.PredecessorResultReceipts) != 0 || len(request.Tools) != 0 {
		return false
	}
	valid := uint64(len(request.SystemPrompt)) == request.SystemPromptBytes &&
		uint64(len(request.UserPrompt)) == request.UserPromptBytes &&
		byteDigest(request.SystemPrompt) == request.SystemPromptSHA256 &&
		byteDigest(request.UserPrompt) == request.UserPromptSHA256 &&
		validSystemPrompt(request.SystemPrompt) && validUserPrompt(request.UserPrompt, nodeID)
	digest, err := domainDigest(requestDigestDomain, requestPayloadFrom(request))
	return valid && err == nil && digest == request.RequestSHA256
}

func validSystemPrompt(prompt string) bool {
	if !strings.HasPrefix(prompt, systemPromptPrefix) {
		return false
	}
	instruction := strings.TrimPrefix(prompt, systemPromptPrefix)
	return validProse(instruction, maxProseBytes)
}

func validUserPrompt(prompt, nodeID string) bool {
	data := []byte(prompt)
	if len(data) == 0 || len(data) > 2*maxProseBytes+1_024 ||
		!utf8.Valid(data) || !validUnicodeEscapes(data) ||
		rejectDuplicateFields(data) != nil {
		return false
	}
	if _, err := exactObject(data, userPromptFields); err != nil {
		return false
	}
	var decoded userPrompt
	if json.Unmarshal(data, &decoded) != nil || decoded.V != 1 ||
		decoded.NodeID != nodeID || decoded.PredecessorResultReceipts == nil ||
		len(decoded.PredecessorResultReceipts) != 0 ||
		!validProse(decoded.Task, maxProseBytes) ||
		!validProse(decoded.Acceptance, maxProseBytes) {
		return false
	}
	canonical, err := canonicalBytes(decoded)
	return err == nil && bytes.Equal(data, canonical)
}

func validBudgets(budgets ExecutionBudgets) bool {
	return budgets.MaxTurns == 1 && budgets.MaxToolCalls == 0 &&
		inRange(budgets.MaxOutputTokens, MaxOutputTokens) &&
		inRange(budgets.MaxModelOutputBytes, MaxModelOutputBytes) &&
		inRange(budgets.MaxModelEvents, MaxModelEvents) &&
		inRange(budgets.TimeoutMilliseconds, MaxTimeoutMilliseconds) &&
		inRange(budgets.MaxCostUSDMicros, MaxCostUSDMicros) &&
		isLowerHexDigest(budgets.PricingSnapshotSHA256)
}

func validPolicies(contract NodeExecutionContract) bool {
	approval := contract.Approval
	result := contract.Result
	failure := contract.Failure
	return approval.ProviderDispatch == "fresh_off_machine_consent" &&
		approval.Workspace == "forbidden" && approval.Tools == "forbidden" &&
		approval.Writeback == "forbidden" &&
		result.ArtifactKind == "local_graph_node_artifact" &&
		inRange(result.MaxResultBytes, MaxResultBytes) &&
		result.PredecessorDataflow == "none" &&
		result.ConversationWriteback == "none" && result.PromptWriteback == "none" &&
		result.MemoryWriteback == "none" && !failure.AutomaticRetry &&
		!failure.LeaseRetry && failure.PostClaimUncertainty == "dispatch_unknown" &&
		failure.FailurePropagationOwner == "forge_core"
}

func isLowerHexDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') &&
			(character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func validText(value string, maxBytes int) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" &&
		len(value) <= maxBytes && !strings.ContainsFunc(value, unsupportedCharacter)
}

func validProse(value string, maxBytes int) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" &&
		len(value) <= maxBytes && !strings.ContainsFunc(value, unsupportedProseCharacter)
}

func unsupportedProseCharacter(value rune) bool {
	return (unicode.IsControl(value) && value != '\n' && value != '\r' && value != '\t') ||
		unsupportedBidiCharacter(value)
}

func unsupportedCharacter(value rune) bool {
	return unicode.IsControl(value) || unsupportedBidiCharacter(value)
}

func unsupportedBidiCharacter(value rune) bool {
	return value == '\u061c' ||
		value == '\u200e' || value == '\u200f' ||
		(value >= '\u2028' && value <= '\u202e') ||
		(value >= '\u2066' && value <= '\u2069')
}
