package graphscheduledcontract

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"forgeos/forge-core/internal/scheduledterminal"
)

// controlCanonicalJSON returns the canonical control snapshot JSON consumed
// by the CLI's DecodeControl.
func controlCanonicalJSON(t *testing.T) []byte {
	t.Helper()
	return []byte(readSourceFixture(t).Input.CanonicalControlSnapshotJSON)
}

func TestReadyCommandListsWaveParallelPlan(t *testing.T) {
	snapshot, scheduleSHA256, frontendReceipt := fixtureSnapshot(t), mustSchedule(t).ScheduleSHA256, successorReceipt(t)
	encodedReceipt, err := scheduledterminal.MarshalReceipt(frontendReceipt)
	if err != nil {
		t.Fatalf("marshal receipt: %v", err)
	}
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	receiptPath := writeTemp(t, dir, "receipt.json", encodedReceipt)
	var stdout, stderr bytes.Buffer
	code := ReadyCommand(
		[]string{
			"--control", controlPath,
			"--schedule-sha256", scheduleSHA256,
			"--predecessor-receipt", receiptPath,
		},
		strings.NewReader(""),
		&stdout, &stderr,
	)
	if code != 0 {
		t.Fatalf("ReadyCommand = %d, stderr %q", code, stderr.String())
	}
	var ready []string
	if err := json.Unmarshal(stdout.Bytes(), &ready); err != nil {
		t.Fatalf("decode ready list: %v", err)
	}
	if len(ready) != 1 {
		t.Fatalf("ready = %v, want exactly one node after frontend", ready)
	}
	if ready[0] == frontendReceipt.NodeID {
		t.Fatalf("consumed node %s must not be ready", ready[0])
	}
	_ = snapshot
}

func TestReadyCommandWithoutReceiptsListsZeroPredecessorSibling(t *testing.T) {
	schedule := mustSchedule(t)
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	var stdout, stderr bytes.Buffer
	code := ReadyCommand(
		[]string{"--control", controlPath, "--schedule-sha256", schedule.ScheduleSHA256},
		strings.NewReader(""),
		&stdout, &stderr,
	)
	var ready []string
	if err := json.Unmarshal(stdout.Bytes(), &ready); code != 0 || err != nil ||
		len(ready) != 1 || ready[0] != schedule.Nodes[1].NodeID {
		t.Fatalf("ReadyCommand empty set: code=%d ready=%v stderr=%q err=%v", code, ready, stderr.String(), err)
	}
}

func TestReadyCommandRejectsAmbiguousStdinAndTrailingArguments(t *testing.T) {
	digest := mustSchedule(t).ScheduleSHA256
	for _, args := range [][]string{
		{"--control", "-", "--schedule-sha256", digest, "--predecessor-receipt", "-"},
		{"--control", "-", "--schedule-sha256", digest, "trailing"},
		{"--control", "-", "--schedule-sha256", strings.ToUpper(digest)},
	} {
		var stdout, stderr bytes.Buffer
		if code := ReadyCommand(args, strings.NewReader(""), &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("args=%v code=%d stdout=%q stderr=%q", args, code, stdout.String(), stderr.String())
		}
	}
}

func TestContractCommandTargetNodeAllowsZeroReceipts(t *testing.T) {
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	args := append(
		fixtureCommandArgs(options, controlPath),
		"--target-node", schedule.Nodes[1].NodeID,
	)
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(""), &stdout, &stderr)
	candidate, err := DecodeCandidate(bytes.NewReader(stdout.Bytes()))
	if code != 0 || err != nil || candidate.ContractScope != successorContractScope ||
		candidate.Node.NodeID != schedule.Nodes[1].NodeID ||
		len(candidate.Request.PredecessorTerminalReceipts) != 0 {
		t.Fatalf("zero-receipt target: code=%d candidate=%#v stderr=%q err=%v", code, candidate, stderr.String(), err)
	}
}

func TestContractCommandTargetNodeRejectsMissingDirectReceipts(t *testing.T) {
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	args := append(
		fixtureCommandArgs(options, controlPath),
		"--target-node", schedule.Nodes[2].NodeID,
	)
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(""), &stdout, &stderr)
	if code == 0 || stdout.Len() != 0 {
		t.Fatalf("missing direct receipts: code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestReadyCommandRejectsDriftedReceipt(t *testing.T) {
	scheduleSHA256 := mustSchedule(t).ScheduleSHA256
	drifted := successorReceipt(t)
	drifted.NodeID = "not-a-node"
	encodedReceipt, err := scheduledterminal.MarshalReceipt(drifted)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	receiptPath := writeTemp(t, dir, "receipt.json", encodedReceipt)
	var stdout, stderr bytes.Buffer
	code := ReadyCommand(
		[]string{"--control", controlPath, "--schedule-sha256", scheduleSHA256, "--predecessor-receipt", receiptPath},
		strings.NewReader(""),
		&stdout, &stderr,
	)
	if code == 0 {
		t.Fatal("ReadyCommand must reject a receipt for an unknown node")
	}
	if !strings.Contains(stderr.String(), "cannot compute ready successor nodes") {
		t.Fatalf("stderr must report the planning fault: %q", stderr.String())
	}
}

func TestContractCommandTargetNodeSelectsSpecificSuccessor(t *testing.T) {
	schedule := mustSchedule(t)
	if schedule.NodeCount < 3 {
		t.Skip("target-node test needs >= 3 nodes")
	}

	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	receiptPath := writeTemp(t, dir, "receipt.json", encoded)
	args := []string{
		"--control", controlPath, "--schedule-sha256", schedule.ScheduleSHA256,
		"--endpoint", options.Endpoint, "--model", options.Model,
		"--max-output-tokens", "4096", "--max-model-output-bytes", "65536",
		"--max-model-events", "4096", "--timeout-ms", "300000",
		"--max-cost-usd-micros", "1000000", "--pricing-snapshot-sha256", options.PricingSnapshotSHA256,
		"--max-result-bytes", "262144",
		"--predecessor-receipt", receiptPath,
		"--target-node", schedule.Nodes[1].NodeID,
	}
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(""), &stdout, &stderr)
	if code != 0 {
		t.Fatalf("command code=%d stderr=%q", code, stderr.String())
	}
	candidate, err := DecodeCandidate(bytes.NewReader(stdout.Bytes()))
	if err != nil {
		t.Fatalf("decode candidate: %v", err)
	}
	if candidate.Node.NodeID != schedule.Nodes[1].NodeID {
		t.Fatalf("target node = %s, want %s", candidate.Node.NodeID, schedule.Nodes[1].NodeID)
	}
}

func TestContractCommandTargetNodeRejectsNotReadyNode(t *testing.T) {
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipt := successorReceipt(t)
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	dir := t.TempDir()
	controlPath := writeTemp(t, dir, "control.json", controlCanonicalJSON(t))
	receiptPath := writeTemp(t, dir, "receipt.json", encoded)
	// node 0 is consumed; targeting it again must fail.
	args := []string{
		"--control", controlPath, "--schedule-sha256", schedule.ScheduleSHA256,
		"--endpoint", options.Endpoint, "--model", options.Model,
		"--max-output-tokens", "4096", "--max-model-output-bytes", "65536",
		"--max-model-events", "4096", "--timeout-ms", "300000",
		"--max-cost-usd-micros", "1000000", "--pricing-snapshot-sha256", options.PricingSnapshotSHA256,
		"--max-result-bytes", "262144",
		"--predecessor-receipt", receiptPath,
		"--target-node", receipt.NodeID,
	}
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(""), &stdout, &stderr)
	if code == 0 {
		t.Fatal("targeting a consumed node must fail")
	}
}
