package authenticatedadrlifecycleauthority

import "bytes"

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value...)
}

func clearBytes(value []byte) {
	for index := range value {
		value[index] = 0
	}
}

func clearMatrix(values [][]byte) {
	for _, value := range values {
		clearBytes(value)
	}
}

func exactBytes(left, right []byte) bool {
	return len(left) == len(right) && bytes.Equal(left, right)
}
