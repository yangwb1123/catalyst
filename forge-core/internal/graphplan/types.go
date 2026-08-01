// Package graphplan validates an inert Group Agent Graph v1 specification and
// emits the immutable topology plan owned by forge-core. It never executes a
// provider, tool, workspace operation, or graph node.
package graphplan

const (
	// SpecVersion is the authored Group Agent Graph JSON version.
	SpecVersion uint16 = 1
	// SchedulerProtocolVersion binds the plan to forge-core's scheduler contract.
	SchedulerProtocolVersion uint16 = 1
	// GraphVersion is the frozen graph manifest version consumed by the plan.
	GraphVersion uint16 = 1
	// MaxSpecBytes is the maximum accepted authored spec size.
	MaxSpecBytes = 2 * 1024 * 1024

	maxNodes             = 32
	maxEdges             = 512
	maxIdentifierBytes   = 128
	maxAgentProfileBytes = 128
	maxMemberRoleBytes   = 64
	maxProseBytes        = 64 * 1024
)

const planDigestDomain = "forge.group-agent-graph-core-plan.v1\x00"

// Spec is the complete authored Group Agent Graph v1 input.
type Spec struct {
	V       uint16  `json:"v"`
	Manager Manager `json:"manager"`
	Nodes   []Node  `json:"nodes"`
	Edges   []Edge  `json:"edges"`
}

// Manager carries inert manager profile and instruction labels.
type Manager struct {
	AgentProfile string `json:"agent_profile"`
	Instruction  string `json:"instruction"`
}

// Node is one inert authored graph node.
type Node struct {
	NodeID       string `json:"node_id"`
	ProjectID    string `json:"project_id"`
	MemberRole   string `json:"member_role"`
	AgentProfile string `json:"agent_profile"`
	Task         string `json:"task"`
	Acceptance   string `json:"acceptance"`
}

// Edge is a dependency-order constraint only.
type Edge struct {
	FromNodeID string `json:"from_node_id"`
	ToNodeID   string `json:"to_node_id"`
}

// Plan is forge-core's immutable, non-executable scheduler plan.
type Plan struct {
	V                         uint16     `json:"v"`
	SchedulerProtocolVersion  uint16     `json:"scheduler_protocol_version"`
	GraphVersion              uint16     `json:"graph_version"`
	GraphID                   string     `json:"graph_id"`
	GraphManifestSHA256       string     `json:"graph_manifest_sha256"`
	AuthoredNodeIDs           []string   `json:"authored_node_ids"`
	Edges                     []Edge     `json:"edges"`
	Waves                     [][]string `json:"waves"`
	ExecutionContractPresent  bool       `json:"execution_contract_present"`
	DispatchAuthorityReleased bool       `json:"dispatch_authority_released"`
	PlanSHA256                string     `json:"plan_sha256"`
}

type planPayload struct {
	V                         uint16     `json:"v"`
	SchedulerProtocolVersion  uint16     `json:"scheduler_protocol_version"`
	GraphVersion              uint16     `json:"graph_version"`
	GraphID                   string     `json:"graph_id"`
	GraphManifestSHA256       string     `json:"graph_manifest_sha256"`
	AuthoredNodeIDs           []string   `json:"authored_node_ids"`
	Edges                     []Edge     `json:"edges"`
	Waves                     [][]string `json:"waves"`
	ExecutionContractPresent  bool       `json:"execution_contract_present"`
	DispatchAuthorityReleased bool       `json:"dispatch_authority_released"`
}
