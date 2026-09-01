package domain

import (
	"fmt"
	"sort"
)

func exactPayload(value map[string]any, fields ...string) error {
	if value == nil || len(value) != len(fields) {
		return invalidHistory("payload fields differ")
	}
	want := append([]string(nil), fields...)
	got := make([]string, 0, len(value))
	for key := range value {
		got = append(got, key)
	}
	sort.Strings(want)
	sort.Strings(got)
	for index := range want {
		if want[index] != got[index] {
			return invalidHistory("payload fields differ")
		}
	}
	return nil
}

func payloadString(value map[string]any, field string) (string, error) {
	result, ok := value[field].(string)
	if !ok {
		return "", invalidHistory(fmt.Sprintf("payload %s is not text", field))
	}
	return result, nil
}

func payloadInt64(value map[string]any, field string) (int64, error) {
	switch result := value[field].(type) {
	case int64:
		return result, nil
	case int:
		return int64(result), nil
	default:
		return 0, invalidHistory(fmt.Sprintf("payload %s is not an integer", field))
	}
}

func payloadObject(value map[string]any, field string) (map[string]any, error) {
	result, ok := value[field].(map[string]any)
	if !ok {
		return nil, invalidHistory(fmt.Sprintf("payload %s is not an object", field))
	}
	return result, nil
}
