package bootstraprepoexecutionauthority

import "fmt"

func validateLedgerDocument(document map[string]any, trust *Trust) (*Ledger, error) {
	if err := validateLedgerShape(document, trust); err != nil {
		return nil, err
	}
	entries, _ := arrayValue(document, "entries")
	ledger := &Ledger{document: document, entries: make([]*usageEntry, 0, len(entries)),
		byGrant: make(map[string]*usageGroup), byRecord: make(map[string]*usageGroup),
		trust: trust}
	prefixSizes, err := ledgerPrefixSizes(document, entries)
	if err != nil {
		return nil, err
	}
	highWater, err := replayLedgerEntries(entries, prefixSizes, ledger)
	if err != nil {
		return nil, err
	}
	claimedHighWater, _ := intValue(document, "clock_high_water_unix_ms")
	if claimedHighWater < highWater {
		return nil, fmt.Errorf("UsageLedger clock high-water is below observations")
	}
	if err = validateSigned(document, "ledger_sha256", ledgerDomain, ledgerSignatureDomain,
		maxLedgerBytes, "UsageLedger", trust, "execution_receipt_sign", ""); err != nil {
		return nil, err
	}
	if err = validateActiveQuarantineCapacity(document, ledger); err != nil {
		return nil, err
	}
	return ledger, nil
}

func validateActiveQuarantineCapacity(document map[string]any, ledger *Ledger) error {
	if ledger.active == nil {
		return nil
	}
	encoded, err := canonicalJSON(document)
	if err != nil {
		return err
	}
	if len(encoded)+maxReceiptBytes+orphanOverheadBytes > maxLedgerBytes {
		return fmt.Errorf("active UsageLedger lacks byte capacity for quarantine")
	}
	return nil
}

func validateLedgerShape(document map[string]any, trust *Trust) error {
	if err := requireKeys(document, ledgerKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadUsageLedger: %w", err)
	}
	if err := validateEnvelope(document, ledgerAPI, "BootstrapRepoReadUsageLedger"); err != nil {
		return err
	}
	if document["profile_id"] != profileID || document["trust_epoch"] != trust.epoch ||
		document["trust_root_sha256"] != trust.rootHash {
		return fmt.Errorf("UsageLedger authority binding is invalid")
	}
	if highWater, err := intValue(document, "clock_high_water_unix_ms"); err != nil || highWater < 0 {
		return fmt.Errorf("UsageLedger clock high-water is invalid")
	}
	entries, err := arrayValue(document, "entries")
	if err != nil || len(entries) < 1 || len(entries) > maxLedgerItems {
		return fmt.Errorf("UsageLedger entries must contain 1..%d items", maxLedgerItems)
	}
	return validateHashField(document, "ledger_sha256", "UsageLedger ledger_sha256")
}

func replayLedgerEntries(values []any, prefixSizes []int, ledger *Ledger) (int64, error) {
	var prior *Receipt
	var highWater int64
	for index, value := range values {
		entryNode, ok := value.(map[string]any)
		if !ok || requireKeys(entryNode, ledgerEntryKeys...) != nil ||
			entryNode["sequence"] != int64(index+1) {
			return 0, fmt.Errorf("UsageLedger entry %d shape or sequence is invalid", index)
		}
		entry, err := replayLedgerEntry(entryNode, ledger, prior, prefixSizes[index])
		if err != nil {
			return 0, fmt.Errorf("UsageLedger entry %d: %w", index, err)
		}
		ledger.entries = append(ledger.entries, entry)
		prior = entry.receipt
		recorded, _ := intValue(entry.receipt.document, "recorded_at_unix_ms")
		highWater = maxInt64(highWater, recorded)
		if entry.invocation != nil {
			requested, _ := intValue(entry.invocation.document, "requested_at_unix_ms")
			highWater = maxInt64(highWater, requested)
		}
	}
	return highWater, nil
}

func replayLedgerEntry(node map[string]any, ledger *Ledger, prior *Receipt,
	prefixSize int) (*usageEntry, error) {
	receiptNode, ok := node["receipt"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("embedded UsageReceipt is not an object")
	}
	if err := validateReceipt(receiptNode, ledger.trust); err != nil {
		return nil, err
	}
	receipt := &Receipt{receiptNode}
	if receiptNode["ledger_sequence"] != node["sequence"] ||
		receiptNode["prior_usage_receipt_sha256"] != nullableReceiptHash(prior) {
		return nil, fmt.Errorf("UsageReceipt sequence or global chain is invalid")
	}
	if receiptNode["state"] == "reserved_no_repo_io" {
		return replayReservation(node, receipt, ledger, prior, prefixSize)
	}
	return replayContinuation(node, receipt, ledger, prior)
}

func replayReservation(node map[string]any, receipt *Receipt, ledger *Ledger,
	prior *Receipt, prefixSize int) (*usageEntry, error) {
	if ledger.active != nil || node["result_metadata"] != nil {
		return nil, fmt.Errorf("reservation overlaps an active group or embeds metadata")
	}
	sequence, _ := intValue(receipt.document, "ledger_sequence")
	if sequence > maxLedgerItems-2 {
		return nil, fmt.Errorf("reservation lacks capacity for intent and terminal")
	}
	manifest, policy, invocation, err := replayReservationInputs(node, ledger.trust, prefixSize)
	if err != nil {
		return nil, err
	}
	transition := transitionContext{sequence: receipt.document["ledger_sequence"].(int64), prior: prior}
	if err = validateReceiptTransition(receipt, transition, policy, invocation, manifest, nil); err != nil {
		return nil, err
	}
	if err = validateTransitionTime("reserved_no_repo_io", policy, invocation, prior,
		receipt.document["recorded_at_unix_ms"].(int64)); err != nil {
		return nil, err
	}
	group := &usageGroup{policy: policy, invocation: invocation, manifest: manifest, reservation: receipt}
	if err = indexUsageGroup(ledger, group); err != nil {
		return nil, err
	}
	ledger.active = group
	return &usageEntry{policy: policy, invocation: invocation, manifest: manifest, receipt: receipt}, nil
}

func replayReservationInputs(node map[string]any, trust *Trust,
	prefixSize int) (*Manifest, *Policy, *Invocation, error) {
	manifestBytes, manifestErr := canonicalEmbedded(node, "manifest", maxManifestBytes)
	policyBytes, policyErr := canonicalEmbedded(node, "execution_policy", maxPolicyBytes)
	invocationBytes, invocationErr := canonicalEmbedded(node, "invocation", maxInvocationBytes)
	if manifestErr != nil || policyErr != nil || invocationErr != nil {
		return nil, nil, nil, fmt.Errorf("reservation embedded documents are invalid")
	}
	if err := validateReservationPrefixCapacity(prefixSize, manifestBytes, policyBytes,
		invocationBytes); err != nil {
		return nil, nil, nil, err
	}
	manifest, err := DecodeManifest(manifestBytes)
	if err != nil {
		return nil, nil, nil, err
	}
	policyDocument, err := decodeCanonical(policyBytes, maxPolicyBytes)
	if err != nil {
		return nil, nil, nil, err
	}
	if err = validateReplayPolicy(policyDocument, trust, manifest); err != nil {
		return nil, nil, nil, err
	}
	policy := &Policy{document: policyDocument}
	invocationDocument, err := decodeCanonical(invocationBytes, maxInvocationBytes)
	if err != nil {
		return nil, nil, nil, err
	}
	if err = validateReplayInvocation(invocationDocument, trust, manifest, policy); err != nil {
		return nil, nil, nil, err
	}
	return manifest, policy, &Invocation{invocationDocument}, nil
}

func validateReservationPrefixCapacity(prefixSize int, documents ...[]byte) error {
	reserve := 3*maxReceiptBytes + maxMetadataBytes + reservationOverheadBytes
	for _, document := range documents {
		reserve += len(document)
	}
	if prefixSize+reserve > maxLedgerBytes {
		return fmt.Errorf("reservation did not preflight future ledger byte capacity")
	}
	return nil
}

func replayContinuation(node map[string]any, receipt *Receipt, ledger *Ledger,
	prior *Receipt) (*usageEntry, error) {
	if node["execution_policy"] != nil || node["invocation"] != nil || node["manifest"] != nil ||
		ledger.active == nil {
		return nil, fmt.Errorf("continuation embedding or active group is invalid")
	}
	group := ledger.active
	metadata, err := replayStoredMetadata(node["result_metadata"], receipt, group)
	if err != nil {
		return nil, err
	}
	transition := transitionContext{sequence: receipt.document["ledger_sequence"].(int64),
		prior: prior, reservation: group.reservation, intent: group.intent}
	if err = validateReceiptTransition(receipt, transition, group.policy, group.invocation,
		group.manifest, metadata); err != nil {
		return nil, err
	}
	state := receipt.document["state"].(string)
	if err = validateTransitionTime(state, group.policy, group.invocation, prior,
		receipt.document["recorded_at_unix_ms"].(int64)); err != nil {
		return nil, err
	}
	entry := &usageEntry{receipt: receipt, metadata: metadata}
	if state == "effect_intent" {
		if group.intent != nil {
			return nil, fmt.Errorf("usage group contains duplicate effect intent")
		}
		group.intent = receipt
		return entry, nil
	}
	group.terminal = entry
	ledger.active = nil
	return entry, nil
}

func replayStoredMetadata(value any, receipt *Receipt, group *usageGroup) (*Metadata, error) {
	if receipt.document["state"] != "completed" {
		if value != nil {
			return nil, fmt.Errorf("only completed entry may embed ResultMetadata")
		}
		return nil, nil
	}
	document, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("completed entry lacks ResultMetadata")
	}
	metadata := &Metadata{document}
	if err := validateStoredMetadata(metadata, group.manifest); err != nil {
		return nil, err
	}
	usage := document["observed_usage"].(map[string]any)
	elapsed, _ := intValue(usage, "elapsed_ms")
	action := group.invocation.document["requested_action"].(map[string]any)
	limit, _ := intValue(action["usage"].(map[string]any), "timeout_ms")
	if elapsed > limit {
		return nil, fmt.Errorf("completed metadata exceeds cooperative timeout budget")
	}
	return metadata, nil
}

func ledgerPrefixSizes(document map[string]any, entries []any) ([]int, error) {
	full, err := canonicalJSON(document)
	if err != nil {
		return nil, err
	}
	envelope := cloneDocument(document)
	envelope["entries"] = []any{}
	empty, err := canonicalJSON(envelope)
	if err != nil {
		return nil, err
	}
	prefixes := make([]int, len(entries))
	prefixSize := len(empty)
	for index, entry := range entries {
		if index > 0 {
			prefixes[index] = prefixSize
		}
		encoded, encodeErr := canonicalJSON(entry)
		if encodeErr != nil {
			return nil, encodeErr
		}
		prefixSize += len(encoded)
		if index > 0 {
			prefixSize++
		}
	}
	if prefixSize != len(full) {
		return nil, fmt.Errorf("UsageLedger capacity accounting drifted")
	}
	return prefixes, nil
}

func canonicalEmbedded(node map[string]any, key string, maximum int) ([]byte, error) {
	document, ok := node[key].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an object", key)
	}
	encoded, err := canonicalJSON(document)
	if err != nil || len(encoded) > maximum {
		return nil, fmt.Errorf("%s canonical bytes exceed limit", key)
	}
	return encoded, nil
}

func indexUsageGroup(ledger *Ledger, group *usageGroup) error {
	grant := group.invocation.document["grant_envelope_sha256"].(string)
	record := recordKey(group.invocation.document["idempotency_key"].(string))
	if ledger.byGrant[grant] != nil || ledger.byRecord[record] != nil {
		return fmt.Errorf("UsageLedger reuses Grant envelope or idempotency record key")
	}
	ledger.byGrant[grant], ledger.byRecord[record] = group, group
	return nil
}
