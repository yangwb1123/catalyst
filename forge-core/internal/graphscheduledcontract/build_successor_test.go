package graphscheduledcontract

import (
	"bytes"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/scheduledterminal"
)

// successorReceipt builds one valid ordinal-zero predecessor receipt bound to
// the fixture control and the first scheduled node.
func successorReceipt(t *testing.T) scheduledterminal.Receipt {
	t.Helper()
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	node := schedule.Nodes[0]
	receipt := scheduledterminal.Receipt{
		V: 1, SchedulerProtocolVersion: 1, TerminalReceiptProtocol: 1,
		TerminalControlSHA256: strings.Repeat("a", 64),
		GraphRunID:            snapshot.GraphRunID,
		GraphID:               snapshot.GraphID,
		NodeID:                node.NodeID,
		Attempt:               node.Attempt,
		DispatchID:            "dispatch-successor-fixture",
		ProviderRequestID:     "scheduled-node-provider-request-successor-fixture",
		ProjectLaneSHA256:     node.ProjectLaneSHA256,
		ArtifactKind:          "result",
		ArtifactID:            "scheduled-node-terminal-artifact-" + strings.Repeat("b", 64),
		ArtifactSHA256:        strings.Repeat("b", 64),
		NodeOutcome:           "completed",
		RetryAuthorized:       false,
		LaneReleaseAuthorized: true,
	}
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("marshal receipt: %v", err)
	}
	decoded, err := scheduledterminal.DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("decode receipt: %v", err)
	}
	return decoded
}

func mustSchedule(t *testing.T) graphschedule.ExecutionSchedule {
	t.Helper()
	snapshot := fixtureSnapshot(t)
	schedule, err := graphschedule.Build(snapshot)
	if err != nil {
		t.Fatalf("build schedule: %v", err)
	}
	return schedule
}

func TestBuildSuccessorSelectsOrdinalOne(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	if schedule.NodeCount < 2 {
		t.Fatalf("successor test needs >= 2 nodes, got %d", schedule.NodeCount)
	}
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	candidate, err := BuildSuccessor(snapshot, schedule.ScheduleSHA256, options, []scheduledterminal.Receipt{receipt}, "", "")
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	if candidate.ContractScope != successorContractScope {
		t.Fatalf("scope = %q", candidate.ContractScope)
	}
	if candidate.Node.ExecutionOrdinal != 1 {
		t.Fatalf("ordinal = %d, want 1", candidate.Node.ExecutionOrdinal)
	}
	if candidate.Node.NodeID != schedule.Nodes[1].NodeID {
		t.Fatalf("node = %q, want %q", candidate.Node.NodeID, schedule.Nodes[1].NodeID)
	}
	if len(candidate.Request.PredecessorTerminalReceipts) != 1 {
		t.Fatalf("predecessor receipts = %d, want 1", len(candidate.Request.PredecessorTerminalReceipts))
	}
	if candidate.Request.PredecessorTerminalReceipts[0].TerminalReceiptSHA256 != receipt.ReceiptSHA256 {
		t.Fatal("predecessor receipt digest not bound")
	}
	if candidate.Request.PredecessorContentIncluded {
		t.Fatal("predecessor content must stay excluded")
	}
	if candidate.SuccessorAdvanceAuthorized || candidate.ProgressObserved ||
		candidate.DispatchAuthorityReleased || candidate.ExecutionAuthorityReleased ||
		candidate.ProviderRequestPresent || candidate.LifecycleContractAdmitted {
		t.Fatal("successor candidate carries forbidden authority")
	}
}

func TestBuildSuccessorRejectsDriftedReceipts(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	cases := []struct {
		name   string
		mutate func(*scheduledterminal.Receipt)
	}{
		{"wrong node", func(r *scheduledterminal.Receipt) { r.NodeID = "other-node" }},
		{"wrong graph run", func(r *scheduledterminal.Receipt) { r.GraphRunID = "other-run" }},
		{"wrong lane", func(r *scheduledterminal.Receipt) { r.ProjectLaneSHA256 = strings.Repeat("0", 64) }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := receipt
			test.mutate(&value)
			encoded, err := scheduledterminal.MarshalReceipt(value)
			if err != nil {
				t.Fatalf("mutated receipt must stay marshallable: %v", err)
			}
			decoded, err := scheduledterminal.DecodeReceipt(encoded)
			if err != nil {
				t.Fatalf("decode mutated receipt: %v", err)
			}
			if _, err := BuildSuccessor(
				snapshot, schedule.ScheduleSHA256, options, []scheduledterminal.Receipt{decoded}, "", "",
			); err == nil {
				t.Fatal("BuildSuccessor accepted a drifted receipt")
			}
		})
	}
}

func TestBuildSuccessorRejectsOutOfOrderAndProtocolViolations(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	// Receipt-level protocol violations are rejected by DecodeReceipt before
	// successor selection can even observe them.
	for _, mutate := range []func(*scheduledterminal.Receipt){
		func(r *scheduledterminal.Receipt) { r.Attempt = 2 },
		func(r *scheduledterminal.Receipt) { r.RetryAuthorized = true },
		func(r *scheduledterminal.Receipt) { r.SuccessorAdvanceAuthorized = true },
	} {
		value := receipt
		mutate(&value)
		encoded, err := scheduledterminal.MarshalReceipt(value)
		if err == nil {
			t.Fatal("DecodeReceipt-layer violation unexpectedly marshaled")
		}
		_ = encoded
	}
	// A receipt that is not the schedule prefix (second node as first receipt).
	value := receipt
	value.NodeID = schedule.Nodes[1].NodeID
	value.ProjectLaneSHA256 = schedule.Nodes[1].ProjectLaneSHA256
	encoded, err := scheduledterminal.MarshalReceipt(value)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	decoded, err := scheduledterminal.DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if _, err := BuildSuccessor(snapshot, schedule.ScheduleSHA256, options, []scheduledterminal.Receipt{decoded}, "", ""); err == nil {
		t.Fatal("BuildSuccessor accepted a non-prefix receipt")
	}
}

func TestBuildSuccessorRejectsEmptyAndFullConsumption(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	if _, err := BuildSuccessor(snapshot, schedule.ScheduleSHA256, options, nil, "", ""); err == nil {
		t.Fatal("BuildSuccessor accepted an empty predecessor set")
	}
	receipts := make([]scheduledterminal.Receipt, 0, len(schedule.Nodes))
	for index := range schedule.Nodes {
		receipt := successorReceipt(t)
		receipt.NodeID = schedule.Nodes[index].NodeID
		receipt.ProjectLaneSHA256 = schedule.Nodes[index].ProjectLaneSHA256
		encoded, err := scheduledterminal.MarshalReceipt(receipt)
		if err != nil {
			t.Fatalf("marshal: %v", err)
		}
		decoded, err := scheduledterminal.DecodeReceipt(encoded)
		if err != nil {
			t.Fatalf("decode: %v", err)
		}
		receipts = append(receipts, decoded)
	}
	if _, err := BuildSuccessor(snapshot, schedule.ScheduleSHA256, options, receipts, "", ""); err == nil {
		t.Fatal("BuildSuccessor accepted full consumption with no successor node")
	}
}

func TestCommandBuildsSuccessorFromReceiptFiles(t *testing.T) {
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", []byte(readSourceFixture(t).Input.CanonicalControlSnapshotJSON))
	receiptPath := writeTemp(t, dir, "receipt.json", encoded)
	var stdout, stderr bytes.Buffer
	code := Command([]string{
		"--control", controlPath, "--schedule-sha256", schedule.ScheduleSHA256,
		"--endpoint", options.Endpoint, "--model", options.Model,
		"--max-output-tokens", "4096", "--max-model-output-bytes", "65536",
		"--max-model-events", "4096", "--timeout-ms", "300000",
		"--max-cost-usd-micros", "1000000", "--pricing-snapshot-sha256", options.PricingSnapshotSHA256,
		"--max-result-bytes", "262144",
		"--predecessor-receipt", receiptPath,
	}, strings.NewReader(""), &stdout, &stderr)
	if code != 0 {
		t.Fatalf("command code=%d stderr=%q", code, stderr.String())
	}
	candidate, err := DecodeCandidate(bytes.NewReader(stdout.Bytes()))
	if err != nil {
		t.Fatalf("decode candidate: %v", err)
	}
	if candidate.ContractScope != successorContractScope || candidate.Node.ExecutionOrdinal != 1 {
		t.Fatalf("candidate scope=%q ordinal=%d", candidate.ContractScope, candidate.Node.ExecutionOrdinal)
	}
}

func writeTemp(t *testing.T, dir, name string, data []byte) string {
	t.Helper()
	path := dir + "/" + name
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatalf("write %s: %v", name, err)
	}
	return path
}

func TestBuildSuccessorEmbedsPredecessorContent(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	content := "frontend produced: login flow verified, token refresh works"
	candidate, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options, []scheduledterminal.Receipt{receipt}, content, "",
	)
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	if !candidate.Request.PredecessorContentIncluded {
		t.Fatal("predecessor content flag must be true when content is embedded")
	}
	if !strings.Contains(candidate.Request.UserPrompt, content) {
		t.Fatalf("user prompt must embed the predecessor output, got %q", candidate.Request.UserPrompt)
	}
	if !strings.Contains(candidate.Request.UserPrompt, "predecessor_output") {
		t.Fatal("user prompt must carry the predecessor_output field")
	}
	// 无内容时字段必须省略(向后兼容)
	plain, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options, []scheduledterminal.Receipt{receipt}, "", "",
	)
	if err != nil {
		t.Fatalf("BuildSuccessor plain: %v", err)
	}
	if plain.Request.PredecessorContentIncluded {
		t.Fatal("plain successor must keep predecessor content excluded")
	}
	if strings.Contains(plain.Request.UserPrompt, "predecessor_output") {
		t.Fatal("plain successor prompt must omit the predecessor_output field")
	}
}

func TestBuildSuccessorRejectsOutOfOrderReceiptBeforeInitial(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	// diamond: frontend/backend -> sso。backend 的 receipt 非 serial 前缀:
	// 初始节点未被消费时,successor 选择必须拒绝它。
	backend := schedule.Nodes[1]
	receipt := scheduledterminal.Receipt{
		V: 1, SchedulerProtocolVersion: 1, TerminalReceiptProtocol: 1,
		TerminalControlSHA256: strings.Repeat("a", 64),
		GraphRunID:            snapshot.GraphRunID,
		GraphID:               snapshot.GraphID,
		NodeID:                backend.NodeID,
		Attempt:               backend.Attempt,
		DispatchID:            "dispatch-backend-fixture",
		ProviderRequestID:     "scheduled-node-provider-request-backend",
		ProjectLaneSHA256:     backend.ProjectLaneSHA256,
		ArtifactKind:          "result",
		ArtifactID:            "scheduled-node-terminal-artifact-" + strings.Repeat("b", 64),
		ArtifactSHA256:        strings.Repeat("b", 64),
		NodeOutcome:           "completed",
		RetryAuthorized:       false,
		LaneReleaseAuthorized: true,
	}
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("marshal backend receipt: %v", err)
	}
	decoded, err := scheduledterminal.DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("decode backend receipt: %v", err)
	}
	if _, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options, []scheduledterminal.Receipt{decoded}, "", "",
	); err == nil {
		t.Fatal("successor selection must reject an out-of-order receipt while initial is unconsumed")
	}
}

func TestBuildSuccessorSelectsReadyWaveSibling(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	// wave 并行核心:consumed = {frontend} 时,backend(同 wave、无前驱)立即就绪,
	// 不需要等待任何其它 ordinal —— diamond 的并行分支可独立推进。
	backend := schedule.Nodes[1]
	frontend := schedule.Nodes[0]
	frontendReceipt := scheduledterminal.Receipt{
		V: 1, SchedulerProtocolVersion: 1, TerminalReceiptProtocol: 1,
		TerminalControlSHA256: strings.Repeat("a", 64),
		GraphRunID:            snapshot.GraphRunID,
		GraphID:               snapshot.GraphID,
		NodeID:                frontend.NodeID,
		Attempt:               frontend.Attempt,
		DispatchID:            "dispatch-frontend-fixture",
		ProviderRequestID:     "scheduled-node-provider-request-frontend",
		ProjectLaneSHA256:     frontend.ProjectLaneSHA256,
		ArtifactKind:          "result",
		ArtifactID:            "scheduled-node-terminal-artifact-" + strings.Repeat("c", 64),
		ArtifactSHA256:        strings.Repeat("c", 64),
		NodeOutcome:           "completed",
		RetryAuthorized:       false,
		LaneReleaseAuthorized: true,
	}
	frontendEncoded, err := scheduledterminal.MarshalReceipt(frontendReceipt)
	if err != nil {
		t.Fatalf("marshal frontend receipt: %v", err)
	}
	frontendDecoded, err := scheduledterminal.DecodeReceipt(frontendEncoded)
	if err != nil {
		t.Fatalf("decode frontend receipt: %v", err)
	}
	candidate, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options,
		[]scheduledterminal.Receipt{frontendDecoded}, "", "",
	)
	if err != nil {
		t.Fatalf("wave-parallel selection failed: %v", err)
	}
	if candidate.Node.NodeID != backend.NodeID {
		t.Fatalf("selected node = %q, want backend (ready same-wave sibling)", candidate.Node.NodeID)
	}
}

func TestReadySuccessorNodesListsWaveParallelPlan(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	// 无 receipts(initial 已做后)??ReadySuccessorNodes 需要 receipts 非空。
	// diamond:消费 frontend 后,就绪 = backend(serial 序第一个);消费 backend 后,就绪 = sso。
	frontend := successorReceipt(t)
	ready, err := ReadySuccessorNodes(snapshot, schedule.ScheduleSHA256, []scheduledterminal.Receipt{frontend})
	if err != nil {
		t.Fatalf("ReadySuccessorNodes: %v", err)
	}
	if len(ready) != 1 || ready[0] != schedule.Nodes[1].NodeID {
		t.Fatalf("ready after frontend = %v, want [%s]", ready, schedule.Nodes[1].NodeID)
	}
	// 消费 frontend + backend → 就绪 = sso(链式:frontend->backend->sso 的下一节点)
	backendReceipt := successorReceipt(t)
	backendReceipt.NodeID = schedule.Nodes[1].NodeID
	backendReceipt.ProjectLaneSHA256 = schedule.Nodes[1].ProjectLaneSHA256
	encoded, err := scheduledterminal.MarshalReceipt(backendReceipt)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	backendDecoded, err := scheduledterminal.DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	ready, err = ReadySuccessorNodes(snapshot, schedule.ScheduleSHA256,
		[]scheduledterminal.Receipt{frontend, backendDecoded})
	if err != nil {
		t.Fatalf("ReadySuccessorNodes all-consumed: %v", err)
	}
	if len(ready) != 1 || ready[0] != schedule.Nodes[2].NodeID {
		t.Fatalf("ready after frontend+backend = %v, want [%s]", ready, schedule.Nodes[2].NodeID)
	}
}
