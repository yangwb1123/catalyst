package graphscheduledreconcile

import (
	"bytes"
	"testing"
)

const goldenSnapshot = `{"v":1,"progress_protocol_version":1,` +
	`"graph_run_id":"graph-run-golden","graph_id":"graph-golden",` +
	`"schedule_id":"graph-execution-schedule-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",` +
	`"schedule_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",` +
	`"node_count":2,"execution_mode":"serial","max_in_flight_nodes":1,` +
	`"progression_policy":"completed_contiguous_prefix","attempt_policy":"exactly_one",` +
	`"failure_policy":"fail_fast_no_retry","nodes":[` +
	`{"execution_ordinal":0,"node_id":"build","attempt":1,` +
	`"candidate_id":"scheduled-node-contract-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",` +
	`"candidate_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",` +
	`"provider_request_id":"scheduled-node-provider-request-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",` +
	`"prepared_request_sha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",` +
	`"lifecycle_status":"terminalized","terminal_outcome":"completed",` +
	`"terminal_receipt_sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},` +
	`{"execution_ordinal":1,"node_id":"verify","attempt":1,` +
	`"candidate_id":"scheduled-node-contract-eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",` +
	`"candidate_sha256":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",` +
	`"provider_request_id":null,"prepared_request_sha256":null,` +
	`"lifecycle_status":null,"terminal_outcome":null,"terminal_receipt_sha256":null}],` +
	`"snapshot_sha256":"a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20"}`

const goldenDecision = `{"v":1,"progress_protocol_version":1,` +
	`"graph_run_id":"graph-run-golden",` +
	`"schedule_id":"graph-execution-schedule-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",` +
	`"schedule_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",` +
	`"snapshot_sha256":"a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20",` +
	`"disposition":"ready","next_execution_ordinal":1,"next_node_id":"verify",` +
	`"decision_sha256":"0c5682601d192a19abb1d23d8bb1597c0eacde8fa098a49b4db548fd5bc56af0"}`

func TestGoldenSnapshotAndDecision(t *testing.T) {
	snapshot, err := DecodeSnapshot(bytes.NewBufferString(goldenSnapshot))
	if err != nil {
		t.Fatalf("DecodeSnapshot(golden): %v", err)
	}
	if snapshot.SnapshotSHA256 != "a847c1b486323dc5b31922b579a5586636d7fd83eac1cca03d2722642be46d20" {
		t.Fatalf("snapshot digest = %q", snapshot.SnapshotSHA256)
	}
	decision, err := Reconcile(snapshot)
	if err != nil {
		t.Fatalf("Reconcile(golden): %v", err)
	}
	decisionBytes, err := MarshalDecision(decision)
	if err != nil {
		t.Fatalf("MarshalDecision(golden): %v", err)
	}
	if !bytes.Equal(decisionBytes, []byte(goldenDecision)) {
		t.Fatalf("decision golden mismatch\ngot  %s\nwant %s", decisionBytes, goldenDecision)
	}
}
