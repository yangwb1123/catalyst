package bootstraprepoexecutionauthority

import "fmt"

var metadataKeys = []string{"api_version", "canonicalization", "content_bytes",
	"execution_result_id", "execution_result_sha256", "kind", "manifest_sha256",
	"metadata_sha256", "observed_usage", "read_count", "reads"}
var metadataReadKeys = []string{"content_bytes", "content_sha256", "path"}

// Metadata is a content-free, exact projection of one validated Result.
type Metadata struct{ document map[string]any }

// BuildMetadata derives ledger-safe metadata without raw content.
func BuildMetadata(result *Result) (*Metadata, error) {
	if result == nil {
		return nil, fmt.Errorf("Result is required")
	}
	document, err := metadataDocument(result.document)
	if err != nil {
		return nil, err
	}
	digest, err := selfDigest(metadataDomain, document, "metadata_sha256",
		maxMetadataBytes, "BootstrapRepoReadResultMetadata", false, "")
	if err != nil {
		return nil, err
	}
	document["metadata_sha256"] = digest
	if err = validateMetadata(document, result); err != nil {
		return nil, err
	}
	return &Metadata{document}, nil
}

func decodeMetadata(data []byte, result *Result) (*Metadata, error) {
	if result == nil {
		return nil, fmt.Errorf("Result is required")
	}
	document, err := decodeCanonical(data, maxMetadataBytes)
	if err != nil {
		return nil, err
	}
	if err = validateMetadata(document, result); err != nil {
		return nil, err
	}
	return &Metadata{document}, nil
}

func (metadata *Metadata) canonicalDocument() map[string]any {
	return cloneDocument(metadata.document)
}

func metadataDocument(result map[string]any) (map[string]any, error) {
	reads, _ := arrayValue(result, "reads")
	metadataReads := make([]any, 0, len(reads))
	for _, value := range reads {
		read := value.(map[string]any)
		metadataReads = append(metadataReads, map[string]any{"content_bytes": read["content_bytes"],
			"content_sha256": read["content_sha256"], "path": read["path"]})
	}
	return map[string]any{"api_version": metadataAPI, "canonicalization": canonicalization,
		"content_bytes": result["content_bytes"], "execution_result_id": result["execution_result_id"],
		"execution_result_sha256": result["execution_result_sha256"],
		"kind":                    "BootstrapRepoReadResultMetadata", "manifest_sha256": result["manifest_sha256"],
		"metadata_sha256": "", "observed_usage": cloneNode(result["observed_usage"]),
		"read_count": int64(len(reads)), "reads": metadataReads}, nil
}

func validateMetadata(document map[string]any, result *Result) error {
	if err := requireKeys(document, metadataKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadResultMetadata: %w", err)
	}
	if err := validateEnvelope(document, metadataAPI, "BootstrapRepoReadResultMetadata"); err != nil {
		return err
	}
	for _, field := range []string{"execution_result_sha256", "manifest_sha256", "metadata_sha256"} {
		if err := validateHashField(document, field, "ResultMetadata "+field); err != nil {
			return err
		}
	}
	expected, err := metadataDocument(result.document)
	if err != nil {
		return err
	}
	expected["metadata_sha256"] = document["metadata_sha256"]
	if !sameCanonical(document, expected) {
		return fmt.Errorf("ResultMetadata differs from exact ExecutionResult")
	}
	claimed := document["metadata_sha256"].(string)
	computed, err := selfDigest(metadataDomain, document, "metadata_sha256",
		maxMetadataBytes, "BootstrapRepoReadResultMetadata", false, "")
	if err != nil || claimed != computed {
		return fmt.Errorf("ResultMetadata self digest does not match")
	}
	return validateMetadataShape(document)
}

func validateMetadataShape(document map[string]any) error {
	resultID, idErr := stringValue(document, "execution_result_id")
	resultDigest, digestOK := document["execution_result_sha256"].(string)
	if idErr != nil || !digestOK || resultID != "bootstrap-repo-read-result-"+resultDigest {
		return fmt.Errorf("ResultMetadata execution result identity is invalid")
	}
	reads, readsErr := arrayValue(document, "reads")
	readCount, countErr := intValue(document, "read_count")
	contentBytes, bytesErr := intValue(document, "content_bytes")
	if readsErr != nil || countErr != nil || readCount != int64(len(reads)) || readCount < 1 ||
		readCount > 16 || bytesErr != nil || contentBytes < 0 || contentBytes > maxContentBytes {
		return fmt.Errorf("ResultMetadata counts are invalid")
	}
	for index, value := range reads {
		read, ok := value.(map[string]any)
		if !ok || requireKeys(read, metadataReadKeys...) != nil {
			return fmt.Errorf("ResultMetadata read %d shape is invalid", index)
		}
		if err := validateHashField(read, "content_sha256", "metadata content_sha256"); err != nil {
			return err
		}
	}
	return nil
}

func validateStoredMetadata(metadata *Metadata, manifest *Manifest) error {
	document := metadata.document
	if err := requireKeys(document, metadataKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadResultMetadata: %w", err)
	}
	if err := validateEnvelope(document, metadataAPI, "BootstrapRepoReadResultMetadata"); err != nil {
		return err
	}
	for _, field := range []string{"execution_result_sha256", "manifest_sha256", "metadata_sha256"} {
		if err := validateHashField(document, field, "ResultMetadata "+field); err != nil {
			return err
		}
	}
	claimed := document["metadata_sha256"].(string)
	computed, err := selfDigest(metadataDomain, document, "metadata_sha256", maxMetadataBytes,
		"BootstrapRepoReadResultMetadata", false, "")
	if err != nil || claimed != computed {
		return fmt.Errorf("stored ResultMetadata self digest does not match")
	}
	if err = validateMetadataShape(document); err != nil {
		return err
	}
	return validateStoredMetadataRelations(document, manifest.document)
}

func validateStoredMetadataRelations(metadata, manifest map[string]any) error {
	if metadata["manifest_sha256"] != manifest["manifest_sha256"] {
		return fmt.Errorf("stored ResultMetadata Manifest binding is invalid")
	}
	reads, _ := arrayValue(metadata, "reads")
	entries, _ := arrayValue(manifest, "entries")
	if len(reads) != len(entries) {
		return fmt.Errorf("stored ResultMetadata reads differ from Manifest")
	}
	var total int64
	for index, value := range reads {
		read := value.(map[string]any)
		entry := entries[index].(map[string]any)
		if read["path"] != entry["path"] || read["content_bytes"] != entry["content_bytes"] ||
			read["content_sha256"] != entry["content_sha256"] {
			return fmt.Errorf("stored ResultMetadata read %d differs from Manifest", index)
		}
		count, _ := intValue(read, "content_bytes")
		total += count
	}
	if metadata["content_bytes"] != total {
		return fmt.Errorf("stored ResultMetadata content bytes differ from reads")
	}
	return validateStoredObservedUsage(metadata["observed_usage"], total)
}

func validateStoredObservedUsage(value any, total int64) error {
	usage, ok := value.(map[string]any)
	if !ok || requireKeys(usage, observedUsageKeys...) != nil || usage["call_count"] != int64(1) ||
		usage["cost_usd_micros"] != int64(0) || usage["input_tokens"] != int64(0) ||
		usage["network_bytes"] != int64(0) || usage["output_bytes"] != total ||
		usage["output_tokens"] != int64(0) {
		return fmt.Errorf("stored ResultMetadata observed_usage is invalid")
	}
	elapsed, err := intValue(usage, "elapsed_ms")
	if err != nil || elapsed < 0 || elapsed > maxFreshnessMillis {
		return fmt.Errorf("stored ResultMetadata elapsed_ms is invalid")
	}
	return nil
}
