package approvalrecordcontract

import (
	"fmt"
	"strconv"
)

func validateCanonicalByteLimit(document map[string]any, maximum int, label string) error {
	size, err := measureCanonical(document, 1, maximum)
	if err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	if size > maximum {
		return fmt.Errorf("%s canonical byte length exceeds %d", label, maximum)
	}
	return nil
}

func measureCanonical(value any, depth, maximum int) (int, error) {
	remaining := maximum
	if err := measureCanonicalNode(value, depth, &remaining); err != nil {
		return 0, err
	}
	return maximum - remaining, nil
}

func measureCanonicalNode(value any, depth int, remaining *int) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	switch typed := value.(type) {
	case map[string]any:
		return measureCanonicalObject(typed, depth, remaining)
	case []any:
		return measureCanonicalArray(typed, depth, remaining)
	case string:
		return measureString(typed, remaining)
	case int64:
		return consumeCanonical(len(strconv.FormatInt(typed, 10)), remaining)
	case bool:
		if typed {
			return consumeCanonical(4, remaining)
		}
		return consumeCanonical(5, remaining)
	case nil:
		return consumeCanonical(4, remaining)
	default:
		return fmt.Errorf("cannot canonicalize %T", value)
	}
}

func measureCanonicalObject(object map[string]any, depth int, remaining *int) error {
	if len(object) > maxObjectFields {
		return fmt.Errorf("object field count exceeds %d", maxObjectFields)
	}
	if err := consumeCanonical(2, remaining); err != nil {
		return err
	}
	index := 0
	for key, child := range object {
		if !isASCIISnakeKey(key) {
			return fmt.Errorf("object key %q is not ASCII snake_case", key)
		}
		if index > 0 {
			if err := consumeCanonical(1, remaining); err != nil {
				return err
			}
		}
		if err := measureString(key, remaining); err != nil {
			return err
		}
		if err := consumeCanonical(1, remaining); err != nil {
			return err
		}
		if err := measureCanonicalNode(child, depth+1, remaining); err != nil {
			return err
		}
		index++
	}
	return nil
}

func measureCanonicalArray(array []any, depth int, remaining *int) error {
	if len(array) > maxArrayItems {
		return fmt.Errorf("array item count exceeds %d", maxArrayItems)
	}
	if err := consumeCanonical(2, remaining); err != nil {
		return err
	}
	for index, child := range array {
		if index > 0 {
			if err := consumeCanonical(1, remaining); err != nil {
				return err
			}
		}
		if err := measureCanonicalNode(child, depth+1, remaining); err != nil {
			return err
		}
	}
	return nil
}

func measureString(value string, remaining *int) error {
	if err := validateWireString(value); err != nil {
		return err
	}
	size := len(value) + 2
	for _, character := range value {
		if character == '"' || character == '\\' {
			size++
		}
	}
	return consumeCanonical(size, remaining)
}

func consumeCanonical(amount int, remaining *int) error {
	if amount < 0 || amount > *remaining {
		return fmt.Errorf("canonical JSON exceeds configured byte limit")
	}
	*remaining -= amount
	return nil
}
