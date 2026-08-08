package scheduledterminal

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
)

const (
	terminalProtocol uint16 = 1
	maxControlBytes         = 64 * 1024 * 1024
	maxArtifactBytes        = 1024 * 1024
	maxReceiptBytes         = 64 * 1024
)

const (
	claimDomain    = "forge.group-agent-scheduled-node-claim.v1\x00"
	artifactDomain = "forge.group-agent-scheduled-node-terminal-artifact.v1\x00"
	controlDomain  = "forge.group-agent-scheduled-node-terminal-control.v1\x00"
	receiptDomain  = "forge.group-agent-scheduled-node-terminal-receipt.v1\x00"
)

type terminalArtifact struct {
	V                        uint16 `json:"v"`
	TerminalArtifactProtocol uint16 `json:"terminal_artifact_protocol_version"`
	ArtifactKind             string `json:"artifact_kind"`
	GraphRunID               string `json:"graph_run_id"`
	NodeID                   string `json:"node_id"`
	Attempt                  uint16 `json:"attempt"`
	DispatchID               string `json:"dispatch_id"`
	ProviderRequestID        string `json:"provider_request_id"`
	ClaimEventSHA256         string `json:"claim_event_sha256"`
	AuthorizationSHA256      string `json:"authorization_sha256"`
	ProviderRequestSHA256    string `json:"provider_request_sha256"`
	RequestBodySHA256        string `json:"request_body_sha256"`
	PricingSnapshotSHA256    string `json:"pricing_snapshot_sha256"`
	LaneOwnershipID          string `json:"lane_ownership_id"`
	ProjectLaneSHA256        string `json:"project_lane_sha256"`
	ProviderPollStarted      bool   `json:"provider_poll_started"`
	TerminalSeen             bool   `json:"terminal_seen"`
	StreamEOFSeen            bool   `json:"stream_eof_seen"`
	Classification           string `json:"classification"`
	OutputText               string `json:"output_text"`
	OutputBytes              int    `json:"output_bytes"`
	OutputSHA256             string `json:"output_sha256"`
	UsageObserved            bool   `json:"usage_observed"`
	InputTokens              uint64 `json:"input_tokens"`
	OutputTokens             uint64 `json:"output_tokens"`
	ActualCostCalculated     bool   `json:"actual_cost_calculated"`
	ActualCostUSDMicros      uint64 `json:"actual_cost_usd_micros"`
	RetryAuthorized          bool   `json:"retry_authorized"`
	CreatedAtMS              uint64 `json:"created_at_ms"`
	ArtifactID               string `json:"artifact_id"`
	ArtifactBytes            int    `json:"artifact_bytes"`
	ArtifactSHA256           string `json:"artifact_sha256"`
}

type terminalControl struct {
	V                         uint16           `json:"v"`
	SchedulerProtocolVersion  uint16           `json:"scheduler_protocol_version"`
	TerminalControlProtocol   uint16           `json:"terminal_control_protocol_version"`
	ReleaseControlSnapshotSHA string           `json:"release_control_snapshot_sha256"`
	GraphRunID                string           `json:"graph_run_id"`
	GraphID                   string           `json:"graph_id"`
	NodeID                    string           `json:"node_id"`
	Attempt                   uint16           `json:"attempt"`
	DispatchID                string           `json:"dispatch_id"`
	ProviderRequestID         string           `json:"provider_request_id"`
	AuthorizationSHA256       string           `json:"authorization_sha256"`
	ProviderRequestSHA256     string           `json:"provider_request_sha256"`
	RequestBodySHA256         string           `json:"request_body_sha256"`
	ExpectedLastEventSeq      uint64           `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256   string           `json:"expected_last_event_sha256"`
	ClaimEventSHA256          string           `json:"claim_event_sha256"`
	ProjectLaneSHA256         string           `json:"project_lane_sha256"`
	Artifact                  terminalArtifact `json:"artifact"`
	SnapshotSHA256            string           `json:"snapshot_sha256"`
}

func decodeControl(data []byte) (terminalControl, error) {
	if len(data) == 0 || len(data) > maxControlBytes {
		return terminalControl{}, errors.New("control size is invalid")
	}
	var value terminalControl
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return terminalControl{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return terminalControl{}, errors.New("trailing control data")
	}
	canonical, err := marshalCanonical(value)
	if err != nil || !bytes.Equal(canonical, data) {
		return terminalControl{}, errors.New("control is not canonical")
	}
	return value, nil
}

func (value terminalControl) validate() error {
	if value.V != 1 || value.SchedulerProtocolVersion != 1 || value.TerminalControlProtocol != terminalProtocol || value.Attempt != 1 || value.ExpectedLastEventSeq != 1 {
		return errors.New("control protocol is invalid")
	}
	if !validIdentifier(value.GraphRunID) || !validIdentifier(value.GraphID) || !validIdentifier(value.NodeID) || !validIdentifier(value.DispatchID) || !validIdentifier(value.ProviderRequestID) {
		return errors.New("control identity is invalid")
	}
	if !validDigest(value.ReleaseControlSnapshotSHA) || !validDigest(value.AuthorizationSHA256) || !validDigest(value.ProviderRequestSHA256) || !validDigest(value.RequestBodySHA256) || !validDigest(value.ExpectedLastEventSHA256) || !validDigest(value.ClaimEventSHA256) || !validDigest(value.ProjectLaneSHA256) {
		return errors.New("control digest is invalid")
	}
	if value.GraphRunID != value.Artifact.GraphRunID || value.NodeID != value.Artifact.NodeID || value.Attempt != value.Artifact.Attempt || value.DispatchID != value.Artifact.DispatchID || value.ProviderRequestID != value.Artifact.ProviderRequestID || value.AuthorizationSHA256 != value.Artifact.AuthorizationSHA256 || value.ProviderRequestSHA256 != value.Artifact.ProviderRequestSHA256 || value.RequestBodySHA256 != value.Artifact.RequestBodySHA256 || value.ProjectLaneSHA256 != value.Artifact.ProjectLaneSHA256 || value.ClaimEventSHA256 != value.Artifact.ClaimEventSHA256 {
		return errors.New("control artifact binding is invalid")
	}
	if err := value.Artifact.validate(); err != nil {
		return err
	}
	digest, err := digestWithoutField(value, "snapshot_sha256", controlDomain)
	if err != nil || value.SnapshotSHA256 != digest {
		return errors.New("control identity is invalid")
	}
	return nil
}

func (value terminalArtifact) validate() error {
	if value.V != 1 || value.TerminalArtifactProtocol != terminalProtocol || value.Attempt != 1 || value.ArtifactBytes <= 0 || value.ArtifactBytes > maxArtifactBytes || value.OutputBytes != len([]byte(value.OutputText)) || value.RetryAuthorized {
		return errors.New("artifact shape is invalid")
	}
	if !validIdentifier(value.GraphRunID) || !validIdentifier(value.NodeID) || !validIdentifier(value.DispatchID) || !validIdentifier(value.ProviderRequestID) || !validIdentifier(value.LaneOwnershipID) {
		return errors.New("artifact identity is invalid")
	}
	if !validClassification(value.Classification) {
		return errors.New("artifact classification is invalid")
	}
	resultClass := value.Classification == "completed"
	if value.Classification == "completed" {
		if value.ArtifactKind != "result" || value.OutputText == "" || !value.ProviderPollStarted || !value.TerminalSeen || !value.StreamEOFSeen || !value.UsageObserved || value.InputTokens == 0 || value.OutputTokens == 0 || value.ActualCostCalculated || value.ActualCostUSDMicros != 0 {
			return errors.New("completed artifact is invalid")
		}
	} else if value.Classification == "length" {
		if value.ArtifactKind != "uncertainty" || !value.ProviderPollStarted || !value.TerminalSeen || !value.StreamEOFSeen || !value.UsageObserved || value.InputTokens == 0 || value.OutputTokens == 0 || value.ActualCostCalculated || value.ActualCostUSDMicros != 0 {
			return errors.New("length artifact is invalid")
		}
	} else if value.ArtifactKind != "uncertainty" {
		return errors.New("uncertain artifact is invalid")
	}
	if !resultClass && (value.ActualCostCalculated || value.ActualCostUSDMicros != 0) {
		return errors.New("uncertain artifact cost is invalid")
	}
	if value.Classification == "missing_usage" && (!value.TerminalSeen || !value.StreamEOFSeen || value.UsageObserved) {
		return errors.New("missing-usage artifact is invalid")
	}
	if !resultClass && !value.ProviderPollStarted && (value.TerminalSeen || value.StreamEOFSeen) {
		return errors.New("uncertain artifact chronology is invalid")
	}
	if !validDigest(value.ClaimEventSHA256) || !validDigest(value.AuthorizationSHA256) || !validDigest(value.ProviderRequestSHA256) || !validDigest(value.RequestBodySHA256) || !validDigest(value.PricingSnapshotSHA256) || !validDigest(value.ProjectLaneSHA256) || !validDigest(value.OutputSHA256) {
		return errors.New("artifact digest is invalid")
	}
	if value.OutputSHA256 != digestBytes("forge.group-agent-scheduled-node-terminal-output.v1\x00", []byte(value.OutputText)) {
		return errors.New("artifact output identity is invalid")
	}
	canonical, err := marshalCanonical(value)
	if err != nil || len(canonical) != value.ArtifactBytes || len(canonical) > maxArtifactBytes {
		return errors.New("artifact byte identity is invalid")
	}
	digest, err := digestWithoutField(value, []string{"artifact_id", "artifact_bytes", "artifact_sha256"}, artifactDomain)
	if err != nil || value.ArtifactSHA256 != digest || value.ArtifactID != "scheduled-node-terminal-artifact-"+digest {
		return errors.New("artifact identity is invalid")
	}
	return nil
}

func validClassification(value string) bool {
	switch value {
	case "completed", "length", "provider_error", "http_error", "transport_error", "timeout", "cancelled", "eof_before_terminal", "missing_usage", "tool_call", "protocol_error", "trailing_data", "local_limit":
		return true
	default:
		return false
	}
}

func buildReceipt(control terminalControl) (terminalReceipt, error) {
	if err := control.validate(); err != nil {
		return terminalReceipt{}, err
	}
	outcome := "failed"
	if control.Artifact.Classification == "completed" {
		outcome = "completed"
	} else if !control.Artifact.ProviderPollStarted || !control.Artifact.TerminalSeen {
		outcome = "failed_uncertain"
	}
	kind := "uncertainty"
	if outcome == "completed" {
		kind = "result"
	}
	receipt := terminalReceipt{
		V: 1, SchedulerProtocolVersion: 1, TerminalReceiptProtocol: terminalProtocol,
		TerminalControlSHA256: control.SnapshotSHA256, GraphRunID: control.GraphRunID,
		GraphID: control.GraphID, NodeID: control.NodeID, Attempt: control.Attempt,
		DispatchID: control.DispatchID, ProviderRequestID: control.ProviderRequestID,
		ProjectLaneSHA256: control.ProjectLaneSHA256, ArtifactKind: kind,
		ArtifactID: control.Artifact.ArtifactID, ArtifactSHA256: control.Artifact.ArtifactSHA256,
		NodeOutcome: outcome, RetryAuthorized: false, LaneReleaseAuthorized: true,
		SuccessorAdvanceAuthorized: false,
	}
	digest, err := digestWithoutField(receipt, []string{"receipt_id", "receipt_sha256"}, receiptDomain)
	if err != nil {
		return terminalReceipt{}, err
	}
	receipt.ReceiptSHA256 = digest
	receipt.ReceiptID = "scheduled-node-terminal-receipt-" + digest
	return receipt, nil
}

func marshalReceipt(value terminalReceipt) ([]byte, error) {
	data, err := marshalCanonical(value)
	if err != nil || len(data) == 0 || len(data) > maxReceiptBytes {
		return nil, errors.New("receipt encoding is invalid")
	}
	return data, nil
}

func marshalCanonical(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "")
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	return bytes.TrimSuffix(buffer.Bytes(), []byte{'\n'}), nil
}

func digestWithoutField(value any, fields any, domain string) (string, error) {
	data, err := marshalCanonical(value)
	if err != nil {
		return "", err
	}
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil {
		return "", err
	}
	remove := func(field string) { delete(object, field) }
	switch selected := fields.(type) {
	case string:
		remove(selected)
	case []string:
		for _, field := range selected {
			remove(field)
		}
	default:
		return "", fmt.Errorf("unsupported digest field set")
	}
	data, err = marshalCanonical(object)
	if err != nil {
		return "", err
	}
	return digestBytes(domain, data), nil
}

func digestBytes(domain string, data []byte) string {
	hash := sha256.New()
	_, _ = hash.Write([]byte(domain))
	_, _ = hash.Write(data)
	return hex.EncodeToString(hash.Sum(nil))
}

func validDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func validIdentifier(value string) bool {
	if len(value) == 0 || len(value) > 256 {
		return false
	}
	for _, byte := range []byte(value) {
		if byte <= 0x20 || byte >= 0x7f {
			return false
		}
	}
	return true
}

// Receipt is the exported, validated view of one terminal receipt. It is the
// exact evidence a successor protocol consumes; it never grants successor
// advancement by itself.
type Receipt struct {
	V                          uint16 `json:"v"`
	SchedulerProtocolVersion   uint16 `json:"scheduler_protocol_version"`
	TerminalReceiptProtocol    uint16 `json:"terminal_receipt_protocol_version"`
	TerminalControlSHA256      string `json:"terminal_control_sha256"`
	GraphRunID                 string `json:"graph_run_id"`
	GraphID                    string `json:"graph_id"`
	NodeID                     string `json:"node_id"`
	Attempt                    uint16 `json:"attempt"`
	DispatchID                 string `json:"dispatch_id"`
	ProviderRequestID          string `json:"provider_request_id"`
	ProjectLaneSHA256          string `json:"project_lane_sha256"`
	ArtifactKind               string `json:"artifact_kind"`
	ArtifactID                 string `json:"artifact_id"`
	ArtifactSHA256             string `json:"artifact_sha256"`
	NodeOutcome                string `json:"node_outcome"`
	RetryAuthorized            bool   `json:"retry_authorized"`
	LaneReleaseAuthorized      bool   `json:"lane_release_authorized"`
	SuccessorAdvanceAuthorized bool   `json:"successor_advance_authorized"`
	ReceiptID                  string `json:"receipt_id"`
	ReceiptSHA256              string `json:"receipt_sha256"`
}

// terminalReceipt is the historical private name for the exported Receipt.
type terminalReceipt = Receipt

// DecodeReceipt strictly decodes one canonical terminal receipt and verifies
// its domain-separated digest and protocol/identity fields. It is the
// successor protocol's trust anchor for predecessor evidence.
func DecodeReceipt(data []byte) (Receipt, error) {
	if len(data) == 0 || len(data) > maxReceiptBytes {
		return Receipt{}, errors.New("receipt size is invalid")
	}
	var value Receipt
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return Receipt{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return Receipt{}, errors.New("trailing receipt data")
	}
	canonical, err := marshalCanonical(value)
	if err != nil || !bytes.Equal(canonical, data) {
		return Receipt{}, errors.New("receipt is not canonical")
	}
	if value.V != 1 || value.SchedulerProtocolVersion != 1 ||
		value.TerminalReceiptProtocol != terminalProtocol || value.Attempt != 1 {
		return Receipt{}, errors.New("receipt protocol is invalid")
	}
	if !validIdentifier(value.GraphRunID) || !validIdentifier(value.GraphID) ||
		!validIdentifier(value.NodeID) || !validIdentifier(value.DispatchID) ||
		!validIdentifier(value.ProviderRequestID) || !validIdentifier(value.ArtifactID) {
		return Receipt{}, errors.New("receipt identity is invalid")
	}
	if !validDigest(value.TerminalControlSHA256) || !validDigest(value.ProjectLaneSHA256) ||
		!validDigest(value.ArtifactSHA256) {
		return Receipt{}, errors.New("receipt digest is invalid")
	}
	if value.ArtifactID != "scheduled-node-terminal-artifact-"+value.ArtifactSHA256 {
		return Receipt{}, errors.New("receipt artifact identity is invalid")
	}
	if !validReceiptOutcome(value) {
		return Receipt{}, errors.New("receipt outcome or release facts are invalid")
	}
	if value.RetryAuthorized || value.SuccessorAdvanceAuthorized {
		return Receipt{}, errors.New("receipt carries forbidden authority")
	}
	digest, err := digestWithoutField(value, []string{"receipt_id", "receipt_sha256"}, receiptDomain)
	if err != nil || value.ReceiptSHA256 != digest {
		return Receipt{}, errors.New("receipt identity is invalid")
	}
	if value.ReceiptID != "scheduled-node-terminal-receipt-"+digest {
		return Receipt{}, errors.New("receipt id is invalid")
	}
	return value, nil
}

// BuildReceipt strictly decodes one canonical terminal control and emits the
// validated, content-addressed intermediate receipt. It is the pure core of
// the `graph-scheduled-node-terminal-receipt` command and the trust anchor a
// successor protocol consumes as predecessor evidence.
func BuildReceipt(controlBytes []byte) ([]byte, error) {
	control, err := decodeControl(controlBytes)
	if err != nil {
		return nil, err
	}
	receipt, err := buildReceipt(control)
	if err != nil {
		return nil, err
	}
	return marshalReceipt(receipt)
}

// MarshalReceipt validates the protocol/identity fields of an explicit receipt
// and emits its canonical JSON with a fresh domain-separated digest and ID. It
// is the counterpart of DecodeReceipt for constructing successor test
// evidence; production receipts are derived from validated controls.
func MarshalReceipt(value Receipt) ([]byte, error) {
	if value.V != 1 || value.SchedulerProtocolVersion != 1 ||
		value.TerminalReceiptProtocol != terminalProtocol || value.Attempt != 1 {
		return nil, errors.New("receipt protocol is invalid")
	}
	if !validIdentifier(value.GraphRunID) || !validIdentifier(value.GraphID) ||
		!validIdentifier(value.NodeID) || !validIdentifier(value.DispatchID) ||
		!validIdentifier(value.ProviderRequestID) || !validIdentifier(value.ArtifactID) {
		return nil, errors.New("receipt identity is invalid")
	}
	if !validDigest(value.TerminalControlSHA256) || !validDigest(value.ProjectLaneSHA256) ||
		!validDigest(value.ArtifactSHA256) {
		return nil, errors.New("receipt digest is invalid")
	}
	if value.ArtifactID != "scheduled-node-terminal-artifact-"+value.ArtifactSHA256 {
		return nil, errors.New("receipt artifact identity is invalid")
	}
	if !validReceiptOutcome(value) {
		return nil, errors.New("receipt outcome or release facts are invalid")
	}
	if value.RetryAuthorized || value.SuccessorAdvanceAuthorized {
		return nil, errors.New("receipt carries forbidden authority")
	}
	digest, err := digestWithoutField(value, []string{"receipt_id", "receipt_sha256"}, receiptDomain)
	if err != nil {
		return nil, err
	}
	value.ReceiptSHA256 = digest
	value.ReceiptID = "scheduled-node-terminal-receipt-" + digest
	return marshalReceipt(value)
}

func validReceiptOutcome(value Receipt) bool {
	if !value.LaneReleaseAuthorized {
		return false
	}
	if value.NodeOutcome == "completed" {
		return value.ArtifactKind == "result"
	}
	return (value.NodeOutcome == "failed" || value.NodeOutcome == "failed_uncertain") &&
		value.ArtifactKind == "uncertainty"
}
