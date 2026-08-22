package kerneloperationalcontract

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"reflect"
)

func canonicalTyped(value any, maximum int) ([]byte, error) {
	node, err := typedNode(value, maximum)
	if err != nil {
		return nil, err
	}
	return canonicalJSON(node, maximum)
}

func typedDigest(value any, domain string, maximum int) (string, error) {
	canonical, err := canonicalTyped(value, maximum)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write(canonical)
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

func cloneValue[T any](value *T) (*T, error) {
	// encoding/json replaces invalid UTF-8 in Go strings with U+FFFD. Validate
	// the caller-owned typed tree before marshaling so cloning can never turn a
	// rejected wire value into a different, valid value.
	if err := validateTypedValue(reflect.ValueOf(value), 1); err != nil {
		return nil, err
	}
	data, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	var result T
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

func artifactReceiptDigest(value *ArtifactReceipt) (string, error) {
	blank, err := cloneValue(value)
	if err != nil {
		return "", err
	}
	blank.ArtifactReceiptID, blank.ArtifactReceiptSHA256 = "", ""
	if err := validateArtifactReceiptFields(blank, true); err != nil {
		return "", err
	}
	return typedDigest(blank, artifactReceiptDomain, maxArtifactReceiptBytes)
}

func validateArtifactReceipt(value *ArtifactReceipt) error {
	if err := validateArtifactReceiptFields(value, false); err != nil {
		return err
	}
	digest, err := artifactReceiptDigest(value)
	if err != nil {
		return err
	}
	if value.ArtifactReceiptSHA256 != digest {
		return fmt.Errorf("artifact_receipt_sha256 does not match canonical preimage")
	}
	_, err = canonicalTyped(value, maxArtifactReceiptBytes)
	return err
}

// SealArtifactReceipt seals one exact blank-identity ArtifactReceipt copy.
func SealArtifactReceipt(value *ArtifactReceipt) (*ArtifactReceipt, error) {
	if value == nil || value.ArtifactReceiptID != "" || value.ArtifactReceiptSHA256 != "" {
		return nil, fmt.Errorf("sealing ArtifactReceipt requires blank identity fields")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := artifactReceiptDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.ArtifactReceiptID = artifactReceiptPrefix + digest
	sealed.ArtifactReceiptSHA256 = digest
	return sealed, validateArtifactReceipt(sealed)
}

// DecodeArtifactReceipt decodes one exact canonical sealed ArtifactReceipt.
func DecodeArtifactReceipt(data []byte) (*ArtifactReceipt, error) {
	var value ArtifactReceipt
	if err := decodeTypedExact(data, maxArtifactReceiptBytes, &value); err != nil {
		return nil, err
	}
	return &value, validateArtifactReceipt(&value)
}

func invocationDigest(value *CapabilityInvocation) (string, error) {
	blank, err := cloneValue(value)
	if err != nil {
		return "", err
	}
	blank.InvocationID, blank.InvocationSHA256 = "", ""
	if err := validateInvocationFields(blank, true); err != nil {
		return "", err
	}
	return typedDigest(blank, invocationDomain, maxInvocationBytes)
}

func validateInvocation(value *CapabilityInvocation) error {
	if err := validateInvocationFields(value, false); err != nil {
		return err
	}
	digest, err := invocationDigest(value)
	if err != nil {
		return err
	}
	if value.InvocationSHA256 != digest {
		return fmt.Errorf("invocation_sha256 does not match canonical preimage")
	}
	_, err = canonicalTyped(value, maxInvocationBytes)
	return err
}

// SealCapabilityInvocation seals one exact blank-identity invocation copy.
func SealCapabilityInvocation(value *CapabilityInvocation) (*CapabilityInvocation, error) {
	if value == nil || value.InvocationID != "" || value.InvocationSHA256 != "" {
		return nil, fmt.Errorf("sealing invocation requires blank identity fields")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := invocationDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.InvocationID, sealed.InvocationSHA256 = invocationPrefix+digest, digest
	return sealed, validateInvocation(sealed)
}

// DecodeCapabilityInvocation decodes one exact canonical sealed invocation.
func DecodeCapabilityInvocation(data []byte) (*CapabilityInvocation, error) {
	var value CapabilityInvocation
	if err := decodeTypedExact(data, maxInvocationBytes, &value); err != nil {
		return nil, err
	}
	return &value, validateInvocation(&value)
}

func eventDigest(value *InteractionEvent) (string, error) {
	blank, err := cloneValue(value)
	if err != nil {
		return "", err
	}
	blank.EventID, blank.EventSHA256 = "", ""
	if err := validateEventFields(blank, true); err != nil {
		return "", err
	}
	return typedDigest(blank, eventDomain, maxEventBytes)
}

func validateEvent(value *InteractionEvent) error {
	if err := validateEventFields(value, false); err != nil {
		return err
	}
	digest, err := eventDigest(value)
	if err != nil {
		return err
	}
	if value.EventSHA256 != digest {
		return fmt.Errorf("event_sha256 does not match canonical preimage")
	}
	_, err = canonicalTyped(value, maxEventBytes)
	return err
}

// SealInteractionEvent seals one exact blank-identity event copy.
func SealInteractionEvent(value *InteractionEvent) (*InteractionEvent, error) {
	if value == nil || value.EventID != "" || value.EventSHA256 != "" {
		return nil, fmt.Errorf("sealing event requires blank identity fields")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := eventDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.EventID, sealed.EventSHA256 = eventPrefix+digest, digest
	return sealed, validateEvent(sealed)
}

// DecodeInteractionEvent decodes one exact canonical sealed event.
func DecodeInteractionEvent(data []byte) (*InteractionEvent, error) {
	var value InteractionEvent
	if err := decodeTypedExact(data, maxEventBytes, &value); err != nil {
		return nil, err
	}
	return &value, validateEvent(&value)
}

func executionReceiptDigest(value *ExecutionReceipt) (string, error) {
	blank, err := cloneValue(value)
	if err != nil {
		return "", err
	}
	blank.ExecutionReceiptID, blank.ExecutionReceiptSHA256 = "", ""
	if err := validateExecutionReceiptFields(blank, true); err != nil {
		return "", err
	}
	return typedDigest(blank, executionReceiptDomain, maxExecutionReceiptBytes)
}

func validateExecutionReceipt(value *ExecutionReceipt) error {
	if err := validateExecutionReceiptFields(value, false); err != nil {
		return err
	}
	digest, err := executionReceiptDigest(value)
	if err != nil {
		return err
	}
	if value.ExecutionReceiptSHA256 != digest {
		return fmt.Errorf("execution_receipt_sha256 does not match canonical preimage")
	}
	_, err = canonicalTyped(value, maxExecutionReceiptBytes)
	return err
}

// SealExecutionReceipt seals one exact blank-identity execution receipt copy.
func SealExecutionReceipt(value *ExecutionReceipt) (*ExecutionReceipt, error) {
	if value == nil || value.ExecutionReceiptID != "" || value.ExecutionReceiptSHA256 != "" {
		return nil, fmt.Errorf("sealing ExecutionReceipt requires blank identity fields")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := executionReceiptDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.ExecutionReceiptID = executionReceiptPrefix + digest
	sealed.ExecutionReceiptSHA256 = digest
	return sealed, validateExecutionReceipt(sealed)
}

// DecodeExecutionReceipt decodes one exact canonical sealed execution receipt.
func DecodeExecutionReceipt(data []byte) (*ExecutionReceipt, error) {
	var value ExecutionReceipt
	if err := decodeTypedExact(data, maxExecutionReceiptBytes, &value); err != nil {
		return nil, err
	}
	return &value, validateExecutionReceipt(&value)
}

// DecodeArtifactRef decodes the exact reused ArtifactRef value object.
func DecodeArtifactRef(data []byte) (*ArtifactRef, error) {
	var value ArtifactRef
	if err := decodeTypedExact(data, maxArtifactRefBytes, &value); err != nil {
		return nil, err
	}
	return &value, validateArtifact(value, "ArtifactRef")
}
