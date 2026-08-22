package bootstraprepoexecutionauthority

import "fmt"

var deliveryKeys = []string{"api_version", "canonicalization", "delivery_disposition",
	"execution_result", "kind", "receipt", "result_metadata"}

type delivery struct{ document map[string]any }

// BuildDelivery creates either raw first delivery or content-free exact replay.
func BuildDelivery(disposition string, result *Result, receipt *Receipt,
	metadata *Metadata) (interface{ canonicalDocument() map[string]any }, error) {
	if receipt == nil {
		return nil, fmt.Errorf("terminal Receipt is required")
	}
	var resultNode any
	var metadataNode any
	switch disposition {
	case "first_delivery":
		if result == nil || metadata == nil || receipt.document["state"] != "completed" {
			return nil, fmt.Errorf("first delivery requires completed Result, Receipt, and Metadata")
		}
		if result.document["execution_result_sha256"] != metadata.document["execution_result_sha256"] {
			return nil, fmt.Errorf("first delivery Result and metadata differ")
		}
		resultNode, metadataNode = cloneNode(result.document), cloneNode(metadata.document)
	case "exact_replay":
		if result != nil || !oneOf(receipt.document["state"].(string), "completed",
			"failed_consumed", "quarantined") {
			return nil, fmt.Errorf("exact replay forbids raw ExecutionResult")
		}
		if receipt.document["state"] == "completed" {
			if metadata == nil {
				return nil, fmt.Errorf("completed replay requires ResultMetadata")
			}
			metadataNode = cloneNode(metadata.document)
		} else if metadata != nil {
			return nil, fmt.Errorf("failed or quarantined replay forbids ResultMetadata")
		}
	default:
		return nil, fmt.Errorf("delivery disposition is unsupported")
	}
	if err := validateDeliveryRelations(receipt, metadata); err != nil {
		return nil, err
	}
	document := map[string]any{"api_version": deliveryAPI, "canonicalization": canonicalization,
		"delivery_disposition": disposition, "execution_result": resultNode,
		"kind": "BootstrapRepoReadExecutionDelivery", "receipt": cloneNode(receipt.document),
		"result_metadata": metadataNode}
	if err := validateDelivery(document); err != nil {
		return nil, err
	}
	return &delivery{document}, nil
}

func validateDeliveryRelations(receipt *Receipt, metadata *Metadata) error {
	if metadata == nil {
		if receipt.document["execution_result_sha256"] != nil ||
			receipt.document["result_metadata_sha256"] != nil {
			return fmt.Errorf("content-free terminal Receipt fields are invalid")
		}
		return nil
	}
	if receipt.document["execution_result_sha256"] != metadata.document["execution_result_sha256"] ||
		receipt.document["result_metadata_sha256"] != metadata.document["metadata_sha256"] {
		return fmt.Errorf("delivery Receipt and ResultMetadata differ")
	}
	return nil
}

func (value *delivery) canonicalDocument() map[string]any { return cloneDocument(value.document) }

func validateDelivery(document map[string]any) error {
	if err := requireKeys(document, deliveryKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadExecutionDelivery: %w", err)
	}
	if err := validateEnvelope(document, deliveryAPI, "BootstrapRepoReadExecutionDelivery"); err != nil {
		return err
	}
	disposition, err := stringValue(document, "delivery_disposition")
	if err != nil || !oneOf(disposition, "first_delivery", "exact_replay") {
		return fmt.Errorf("delivery disposition is invalid")
	}
	if (disposition == "first_delivery") != (document["execution_result"] != nil) {
		return fmt.Errorf("delivery raw result presence is invalid")
	}
	canonical, err := canonicalJSON(document)
	if err != nil || len(canonical) > maxDeliveryBytes {
		return fmt.Errorf("delivery canonical bytes exceed limit")
	}
	return nil
}
