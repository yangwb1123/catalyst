package contextpackagecontract

import (
	"bytes"
	"fmt"
	"unicode/utf8"
)

func redactionsBySource(redactions []Redaction) map[string][]RedactionRange {
	result := make(map[string][]RedactionRange, len(redactions))
	for _, redaction := range redactions {
		result[redaction.SourceID] = redaction.Ranges
	}
	return result
}

func applyRedactions(content string, ranges []RedactionRange) string {
	if len(ranges) == 0 {
		return content
	}
	raw := []byte(content)
	buffer := bytes.NewBuffer(make([]byte, 0, len(raw)))
	position := uint64(0)
	for _, item := range ranges {
		buffer.Write(raw[position:item.StartByte])
		buffer.WriteString(redactionMarker)
		position = item.EndByte
	}
	buffer.Write(raw[position:])
	return buffer.String()
}

func truncateUTF8Prefix(content string, maximum uint64) (string, error) {
	if uint64(len(content)) <= maximum {
		return content, nil
	}
	boundary := maximum
	for boundary > 0 && !utf8.RuneStart(content[boundary]) {
		boundary--
	}
	if boundary == 0 {
		return "", nil
	}
	prefix := content[:boundary]
	if !utf8.ValidString(prefix) {
		return "", fmt.Errorf("computed truncation is not valid UTF-8")
	}
	return prefix, nil
}

func cloneRedactionReceipts(redactions []Redaction) []RedactionReceipt {
	receipts := make([]RedactionReceipt, len(redactions))
	for index, redaction := range redactions {
		ranges := append([]RedactionRange(nil), redaction.Ranges...)
		receipts[index] = RedactionReceipt{SourceID: redaction.SourceID, Ranges: ranges}
	}
	return receipts
}
