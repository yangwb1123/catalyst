package bootstraprepoexecutionauthority

import "fmt"

// CanonicalJSON returns detached byte-stable canonical JSON for a validated document.
func CanonicalJSON(document interface{ canonicalDocument() map[string]any }) ([]byte, error) {
	if document == nil {
		return nil, fmt.Errorf("Document is required")
	}
	return canonicalJSON(document.canonicalDocument())
}

func cloneDocument(document map[string]any) map[string]any {
	return cloneNode(document).(map[string]any)
}
