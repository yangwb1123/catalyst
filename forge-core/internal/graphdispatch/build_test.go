package graphdispatch

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphplan"
)

func TestBuildSelectsOnlyFirstWaveNodeUnderGlobalSingleFlight(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot := decodeFixtureSnapshot(t, fixture)
	snapshot.Manifest.Nodes[1].ProjectID = snapshot.Manifest.Nodes[0].ProjectID
	snapshot.Manifest.Nodes[1].MemberRole = snapshot.Manifest.Nodes[0].MemberRole
	resignSnapshot(t, &snapshot)
	contract, err := Build(snapshot, fixture.Input.ExecutionOptions.options())
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	if len(snapshot.Plan.Waves[0]) != 2 || contract.Node.NodeID != "frontend" {
		t.Fatalf("selection = %#v from wave %#v", contract.Node, snapshot.Plan.Waves[0])
	}
	if contract.Node.SameProjectPolicy != "exclusive_until_terminal" ||
		contract.DispatchAuthorityReleased || contract.Workspace.Mode != "none" {
		t.Fatalf("unsafe first-node contract: %#v", contract)
	}
}

func TestBuildSelectsFirstEligibleNodeWithAuthoredIndex(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot := decodeFixtureSnapshot(t, fixture)
	snapshot.Manifest.Nodes = snapshot.Manifest.Nodes[:2]
	snapshot.Manifest.Edges = []graphplan.Edge{
		{FromNodeID: "backend", ToNodeID: "frontend"},
	}
	snapshot.Manifest.Waves = [][]string{{"backend"}, {"frontend"}}
	resignSnapshot(t, &snapshot)
	contract, err := Build(snapshot, fixture.Input.ExecutionOptions.options())
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	if contract.Node.NodeID != "backend" || contract.Node.AuthoredNodeIndex != 1 ||
		contract.Node.TopologyWaveIndex != 0 {
		t.Fatalf("selection = %#v", contract.Node)
	}
}

func TestBuildRejectsControlBindingDrift(t *testing.T) {
	fixture := readSharedFixture(t)
	tests := []struct {
		name   string
		mutate func(*ControlSnapshot)
	}{
		{"run version", func(s *ControlSnapshot) { s.GraphRunVersion = 2 }},
		{"event cursor", func(s *ControlSnapshot) { s.LastEventSeq = 2 }},
		{"contract flag", func(s *ControlSnapshot) { s.ExecutionContractPresent = true }},
		{"authority flag", func(s *ControlSnapshot) { s.DispatchAuthorityReleased = true }},
		{"source digest", func(s *ControlSnapshot) { s.SourceSnapshotSHA256 = repeatedHex("a") }},
		{"manifest digest", func(s *ControlSnapshot) { s.GraphManifestSHA256 = repeatedHex("a") }},
		{"plan digest", func(s *ControlSnapshot) { s.CorePlanSHA256 = repeatedHex("a") }},
		{"plan topology", func(s *ControlSnapshot) { s.Plan.Waves[0][0] = "backend" }},
		{"manifest task", func(s *ControlSnapshot) { s.Manifest.Nodes[0].Task = "drift" }},
		{"manifest waves", func(s *ControlSnapshot) { s.Manifest.Waves[0][0] = "backend" }},
		{"snapshot digest", func(s *ControlSnapshot) { s.SnapshotSHA256 = repeatedHex("a") }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			snapshot := decodeFixtureSnapshot(t, fixture)
			test.mutate(&snapshot)
			if _, err := Build(snapshot, fixture.Input.ExecutionOptions.options()); err == nil {
				t.Fatal("Build accepted divergent control snapshot")
			}
		})
	}
}

func TestExecutionOptionBoundsFailClosed(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot := decodeFixtureSnapshot(t, fixture)
	tests := invalidOptionCases()
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			options := fixture.Input.ExecutionOptions.options()
			test.mutate(&options)
			if _, err := Build(snapshot, options); err == nil {
				t.Fatal("Build accepted invalid execution options")
			}
		})
	}
	maximum := maximumOptions()
	if _, err := Build(snapshot, maximum); err != nil {
		t.Fatalf("Build rejected exact maxima: %v", err)
	}
}

func TestMarshalContractRejectsAuthorityAndNestedDrift(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot := decodeFixtureSnapshot(t, fixture)
	base, err := Build(snapshot, fixture.Input.ExecutionOptions.options())
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	tests := []struct {
		name   string
		mutate func(*NodeExecutionContract)
	}{
		{"lane", func(c *NodeExecutionContract) { c.Node.ProjectLaneSHA256 = repeatedHex("a") }},
		{"workspace", func(c *NodeExecutionContract) { c.Workspace.Mode = "read_only" }},
		{"provider store", func(c *NodeExecutionContract) { c.Provider.Store = true }},
		{"user Prompt", func(c *NodeExecutionContract) { c.Request.UserPrompt += " " }},
		{"receipt null", func(c *NodeExecutionContract) { c.Request.PredecessorResultReceipts = nil }},
		{"turns", func(c *NodeExecutionContract) { c.Budgets.MaxTurns = 2 }},
		{"approval", func(c *NodeExecutionContract) { c.Approval.Tools = "allowed" }},
		{"result", func(c *NodeExecutionContract) { c.Result.PredecessorDataflow = "copied" }},
		{"retry", func(c *NodeExecutionContract) { c.Failure.AutomaticRetry = true }},
		{"authority", func(c *NodeExecutionContract) { c.DispatchAuthorityReleased = true }},
		{"identity", func(c *NodeExecutionContract) { c.ContractID = "other" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			contract := base
			test.mutate(&contract)
			if _, err := MarshalContract(contract); err == nil {
				t.Fatal("MarshalContract accepted divergent contract")
			}
		})
	}
}

type optionCase struct {
	name   string
	mutate func(*ExecutionOptions)
}

func invalidOptionCases() []optionCase {
	return []optionCase{
		{"http endpoint", func(o *ExecutionOptions) { o.Endpoint = "http://api.example/v1" }},
		{"endpoint userinfo", func(o *ExecutionOptions) { o.Endpoint = "https://secret@api.example/v1" }},
		{"endpoint query", func(o *ExecutionOptions) { o.Endpoint = "https://api.example/v1?key=x" }},
		{"endpoint fragment", func(o *ExecutionOptions) { o.Endpoint = "https://api.example/v1#x" }},
		{"endpoint dot segment", func(o *ExecutionOptions) { o.Endpoint = "https://api.example/v1/../x" }},
		{"endpoint encoded dot", func(o *ExecutionOptions) { o.Endpoint = "https://api.example/v1/%2e%2e/x" }},
		{"endpoint uppercase host", func(o *ExecutionOptions) { o.Endpoint = "https://API.example/v1" }},
		{"endpoint default port", func(o *ExecutionOptions) { o.Endpoint = "https://api.example:443/v1" }},
		{"endpoint too long", func(o *ExecutionOptions) { o.Endpoint = "https://x/" + strings.Repeat("a", MaxEndpointBytes) }},
		{"endpoint utf8 bytes", func(o *ExecutionOptions) { o.Endpoint = "https://x/" + strings.Repeat("é", MaxEndpointBytes/2) }},
		{"model empty", func(o *ExecutionOptions) { o.Model = "" }},
		{"model too long", func(o *ExecutionOptions) { o.Model = strings.Repeat("m", MaxModelBytes+1) }},
		{"model utf8 bytes", func(o *ExecutionOptions) { o.Model = strings.Repeat("é", MaxModelBytes/2+1) }},
		{"tokens zero", func(o *ExecutionOptions) { o.MaxOutputTokens = 0 }},
		{"tokens high", func(o *ExecutionOptions) { o.MaxOutputTokens = MaxOutputTokens + 1 }},
		{"output bytes zero", func(o *ExecutionOptions) { o.MaxModelOutputBytes = 0 }},
		{"output bytes high", func(o *ExecutionOptions) { o.MaxModelOutputBytes = MaxModelOutputBytes + 1 }},
		{"events zero", func(o *ExecutionOptions) { o.MaxModelEvents = 0 }},
		{"events high", func(o *ExecutionOptions) { o.MaxModelEvents = MaxModelEvents + 1 }},
		{"timeout zero", func(o *ExecutionOptions) { o.TimeoutMilliseconds = 0 }},
		{"timeout high", func(o *ExecutionOptions) { o.TimeoutMilliseconds = MaxTimeoutMilliseconds + 1 }},
		{"cost zero", func(o *ExecutionOptions) { o.MaxCostUSDMicros = 0 }},
		{"cost high", func(o *ExecutionOptions) { o.MaxCostUSDMicros = MaxCostUSDMicros + 1 }},
		{"pricing digest", func(o *ExecutionOptions) { o.PricingSnapshotSHA256 = repeatedHex("A") }},
		{"result zero", func(o *ExecutionOptions) { o.MaxResultBytes = 0 }},
		{"result high", func(o *ExecutionOptions) { o.MaxResultBytes = MaxResultBytes + 1 }},
	}
}

func maximumOptions() ExecutionOptions {
	return ExecutionOptions{
		Endpoint: "https://api.example/v1/responses", Model: "m",
		MaxOutputTokens: MaxOutputTokens, MaxModelOutputBytes: MaxModelOutputBytes,
		MaxModelEvents: MaxModelEvents, TimeoutMilliseconds: MaxTimeoutMilliseconds,
		MaxCostUSDMicros: MaxCostUSDMicros, PricingSnapshotSHA256: repeatedHex("f"),
		MaxResultBytes: MaxResultBytes,
	}
}

func resignSnapshot(t *testing.T, snapshot *ControlSnapshot) {
	t.Helper()
	manifestSHA, err := manifestDigest(snapshot.Manifest)
	if err != nil {
		t.Fatalf("manifestDigest: %v", err)
	}
	plan, err := graphplan.Build(graphplan.Spec{
		V: snapshot.Manifest.V, Manager: snapshot.Manifest.Manager,
		Nodes: snapshot.Manifest.Nodes, Edges: snapshot.Manifest.Edges,
	}, snapshot.GraphID, manifestSHA)
	if err != nil {
		t.Fatalf("graphplan.Build: %v", err)
	}
	snapshot.GraphManifestSHA256 = manifestSHA
	snapshot.Plan, snapshot.CorePlanSHA256 = plan, plan.PlanSHA256
	snapshot.SnapshotSHA256, err = domainDigest(snapshotDigestDomain, snapshotPayload(*snapshot))
	if err != nil {
		t.Fatalf("snapshot digest: %v", err)
	}
}

func repeatedHex(character string) string {
	return strings.Repeat(character, 64)
}
