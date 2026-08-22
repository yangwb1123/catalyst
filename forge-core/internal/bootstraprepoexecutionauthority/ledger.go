package bootstraprepoexecutionauthority

import (
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
)

var ledgerKeys = []string{"api_version", "canonicalization", "clock_high_water_unix_ms",
	"entries", "kind", "ledger_sha256", "profile_id", "signature", "trust_epoch",
	"trust_root_sha256"}
var ledgerEntryKeys = []string{"execution_policy", "invocation", "manifest", "receipt",
	"result_metadata", "sequence"}

// Ledger is a fully replayed, signed, complete usage snapshot.
type Ledger struct {
	document map[string]any
	entries  []*usageEntry
	byGrant  map[string]*usageGroup
	byRecord map[string]*usageGroup
	active   *usageGroup
	trust    *Trust
}

type usageEntry struct {
	policy     *Policy
	invocation *Invocation
	manifest   *Manifest
	receipt    *Receipt
	metadata   *Metadata
}

type usageGroup struct {
	policy, terminalPolicy *Policy
	invocation             *Invocation
	manifest               *Manifest
	reservation, intent    *Receipt
	terminal               *usageEntry
}

// DecodeLedger replays every issuance lookup, signature, relation, transition, and chain link.
func DecodeLedger(data []byte, trust *Trust,
	issuance *bootstrapgrantauthority.Ledger) (*Ledger, error) {
	if trust == nil || issuance == nil {
		return nil, fmt.Errorf("execution Trust and issuance Ledger are required")
	}
	if len(data) == 0 {
		return nil, nil
	}
	document, err := decodeCanonical(data, maxLedgerBytes)
	if err != nil {
		return nil, err
	}
	ledger, err := validateLedgerDocument(document, trust)
	if err != nil {
		return nil, err
	}
	if err = validateLedgerIssuance(ledger, issuance); err != nil {
		return nil, err
	}
	return ledger, nil
}

func validateLedgerIssuance(ledger *Ledger, issuance *bootstrapgrantauthority.Ledger) error {
	if issuance == nil {
		return fmt.Errorf("issuance Ledger is required")
	}
	if ledger == nil {
		return nil
	}
	for _, group := range ledger.byGrant {
		grant, err := resolveIssuedGrant(group.policy.document, issuance)
		if err != nil {
			return err
		}
		if err = validateManifestGrant(group.manifest, grant); err != nil {
			return err
		}
		if err = validatePolicyRelations(group.policy.document, ledger.trust,
			grant.document, group.manifest.document); err != nil {
			return err
		}
		if err = validateInvocationGrantRelations(group.invocation.document, grant.document); err != nil {
			return err
		}
		group.policy.grant = grant
	}
	return nil
}

// QuarantineOrphan consumes an active tail using only its embedded authenticated inputs.
func QuarantineOrphan(current *Ledger, issuance *bootstrapgrantauthority.Ledger,
	recordedAt int64, signer *Signer) (*Ledger, *Receipt, bool, error) {
	if current == nil || current.active == nil {
		return current, nil, false, nil
	}
	if err := validateLedgerIssuance(current, issuance); err != nil {
		return nil, nil, true, err
	}
	group := current.active
	reason := "orphaned_reserved_no_repo_io"
	if group.intent != nil {
		reason = "orphaned_effect_intent"
	}
	receipt, err := IssueReceipt(current, "quarantined", group.policy, group.invocation,
		group.manifest, nil, recordedAt, reason, signer)
	if err != nil {
		return nil, nil, true, err
	}
	ledger, err := AppendLedger(current, issuance, group.policy, group.invocation,
		group.manifest, receipt, nil, signer)
	if err != nil {
		return nil, nil, true, err
	}
	return ledger, receipt, true, nil
}

// AppendLedger returns a signed complete snapshot containing the exact next receipt.
func AppendLedger(current *Ledger, issuance *bootstrapgrantauthority.Ledger, policy *Policy,
	invocation *Invocation, manifest *Manifest, receipt *Receipt, metadata *Metadata,
	signer *Signer) (*Ledger, error) {
	if err := validateAppendInputs(current, issuance, policy, invocation, manifest,
		receipt, metadata, signer); err != nil {
		return nil, err
	}
	document, err := appendLedgerDocument(current, policy, invocation, manifest, receipt, metadata, signer.trust)
	if err != nil {
		return nil, err
	}
	digest, err := selfDigest(ledgerDomain, document, "ledger_sha256", maxLedgerBytes,
		"BootstrapRepoReadUsageLedger", true, "")
	if err != nil {
		return nil, err
	}
	document["ledger_sha256"] = digest
	signature := document["signature"].(map[string]any)
	signature["signature_base64url"], err = signer.sign(ledgerSignatureDomain, digest)
	if err != nil {
		return nil, err
	}
	ledger, err := validateLedgerDocument(document, signer.trust)
	if err != nil {
		return nil, err
	}
	if err = validateLedgerIssuance(ledger, issuance); err != nil {
		return nil, err
	}
	return ledger, nil
}

func validateAppendInputs(current *Ledger, issuance *bootstrapgrantauthority.Ledger,
	policy *Policy, invocation *Invocation, manifest *Manifest, receipt *Receipt,
	metadata *Metadata, signer *Signer) error {
	if issuance == nil || policy == nil || invocation == nil || manifest == nil ||
		receipt == nil || signer == nil || signer.trust == nil {
		return fmt.Errorf("complete usage entry, issuance Ledger, and signer are required")
	}
	if current != nil && current.trust.rootHash != signer.trust.rootHash {
		return fmt.Errorf("current Ledger authority inputs differ")
	}
	if err := validateLedgerIssuance(current, issuance); err != nil {
		return err
	}
	issued, err := resolveIssuedGrant(policy.document, issuance)
	if err != nil {
		return err
	}
	if err = validateTransitionInputs(policy, invocation, manifest, signer.trust, issued); err != nil {
		return err
	}
	transition, err := nextTransition(current, receipt.document["state"].(string), policy, invocation, manifest)
	if err != nil {
		return err
	}
	if err = validateReceipt(receipt.document, signer.trust); err != nil {
		return err
	}
	if err = validateReceiptTransition(receipt, transition, policy, invocation, manifest, metadata); err != nil {
		return err
	}
	return nil
}

// Position returns next sequence, prior digest, clock high-water, and active state.
func (ledger *Ledger) Position() (int64, *string, int64, string) {
	if ledger == nil {
		return 1, nil, 0, ""
	}
	last := ledger.entries[len(ledger.entries)-1].receipt.document["receipt_sha256"].(string)
	highWater, _ := intValue(ledger.document, "clock_high_water_unix_ms")
	activeState := ""
	if ledger.active != nil {
		activeState = ledger.active.reservation.document["state"].(string)
		if ledger.active.intent != nil {
			activeState = ledger.active.intent.document["state"].(string)
		}
	}
	return int64(len(ledger.entries) + 1), &last, highWater, activeState
}

func (ledger *Ledger) inspect(invocation *Invocation) (string, *Receipt, *Metadata, bool, bool) {
	if ledger == nil || invocation == nil {
		return "", nil, nil, false, false
	}
	grantKey := invocation.document["grant_envelope_sha256"].(string)
	record := recordKey(invocation.document["idempotency_key"].(string))
	grantGroup, grantFound := ledger.byGrant[grantKey]
	recordGroup, recordFound := ledger.byRecord[record]
	if !grantFound && !recordFound {
		return "", nil, nil, false, false
	}
	if !grantFound || !recordFound || grantGroup != recordGroup ||
		!sameInvocation(grantGroup.invocation, invocation) {
		return "", nil, nil, true, true
	}
	entry := grantGroup.terminal
	if entry == nil {
		last := grantGroup.reservation
		if grantGroup.intent != nil {
			last = grantGroup.intent
		}
		return last.document["state"].(string), last, nil, true, false
	}
	return entry.receipt.document["state"].(string), entry.receipt, entry.metadata, true, false
}

func (ledger *Ledger) canonicalDocument() map[string]any { return cloneDocument(ledger.document) }

func appendLedgerDocument(current *Ledger, policy *Policy, invocation *Invocation,
	manifest *Manifest, receipt *Receipt, metadata *Metadata, trust *Trust) (map[string]any, error) {
	entries := []any{}
	highWater := int64(0)
	if current != nil {
		entries = cloneNode(current.document["entries"]).([]any)
		highWater, _ = intValue(current.document, "clock_high_water_unix_ms")
	}
	if len(entries) >= maxLedgerItems ||
		(receipt.document["state"] == "reserved_no_repo_io" && len(entries) > maxLedgerItems-3) {
		return nil, fmt.Errorf("UsageLedger lacks capacity for a complete usage group")
	}
	entry := usageEntryDocument(policy, invocation, manifest, receipt, metadata)
	entries = append(entries, entry)
	recorded, _ := intValue(receipt.document, "recorded_at_unix_ms")
	requested, _ := intValue(invocation.document, "requested_at_unix_ms")
	highWater = maxInt64(highWater, maxInt64(recorded, requested))
	return map[string]any{"api_version": ledgerAPI, "canonicalization": canonicalization,
		"clock_high_water_unix_ms": highWater, "entries": entries,
		"kind": "BootstrapRepoReadUsageLedger", "ledger_sha256": "", "profile_id": profileID,
		"signature": signaturePlaceholder(trust), "trust_epoch": trust.epoch,
		"trust_root_sha256": trust.rootHash}, nil
}

func usageEntryDocument(policy *Policy, invocation *Invocation, manifest *Manifest,
	receipt *Receipt, metadata *Metadata) map[string]any {
	var policyNode, invocationNode, manifestNode, metadataNode any
	if receipt.document["state"] == "reserved_no_repo_io" {
		policyNode, invocationNode = cloneNode(policy.document), cloneNode(invocation.document)
		manifestNode = cloneNode(manifest.document)
	}
	if metadata != nil {
		metadataNode = cloneNode(metadata.document)
	}
	return map[string]any{"execution_policy": policyNode, "invocation": invocationNode,
		"manifest": manifestNode, "receipt": cloneNode(receipt.document),
		"result_metadata": metadataNode, "sequence": receipt.document["ledger_sequence"]}
}

func activeGroup(ledger *Ledger) *usageGroup {
	if ledger == nil {
		return nil
	}
	return ledger.active
}

func ledgerIdentityUsed(ledger *Ledger, invocation *Invocation) bool {
	if ledger == nil {
		return false
	}
	grant := invocation.document["grant_envelope_sha256"].(string)
	record := recordKey(invocation.document["idempotency_key"].(string))
	return ledger.byGrant[grant] != nil || ledger.byRecord[record] != nil
}

func sameInvocation(left, right *Invocation) bool {
	return left != nil && right != nil &&
		left.document["invocation_sha256"] == right.document["invocation_sha256"]
}

func maxInt64(left, right int64) int64 {
	if left > right {
		return left
	}
	return right
}
