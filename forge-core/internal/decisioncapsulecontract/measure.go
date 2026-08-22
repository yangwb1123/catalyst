package decisioncapsulecontract

import (
	"bytes"
	"encoding"
	"encoding/json"
	"fmt"
	"math"
	"reflect"
	"sort"
	"strconv"
	"strings"
)

var (
	rawMessageType    = reflect.TypeOf(json.RawMessage{})
	jsonNumberType    = reflect.TypeOf(json.Number(""))
	jsonMarshalerType = reflect.TypeOf((*json.Marshaler)(nil)).Elem()
	textMarshalerType = reflect.TypeOf((*encoding.TextMarshaler)(nil)).Elem()
)

type measurementVisit struct {
	kind    reflect.Kind
	typeOf  reflect.Type
	pointer uintptr
}

type typedMeasurement struct {
	maximum int
	seen    map[measurementVisit]struct{}
}

type measuredField struct {
	name  string
	value reflect.Value
}

func measureTypedJSON(value any, maximum int) (int, error) {
	state := typedMeasurement{maximum: maximum, seen: make(map[measurementVisit]struct{})}
	size, err := state.value(reflect.ValueOf(value), 1)
	if err == nil && size > maximum {
		return 0, fmt.Errorf("canonical JSON byte length exceeds %d", maximum)
	}
	return size, err
}

func (state *typedMeasurement) add(total, increment int) (int, error) {
	if increment < 0 || total > state.maximum-increment {
		return 0, fmt.Errorf("canonical JSON byte length exceeds %d", state.maximum)
	}
	return total + increment, nil
}

func (state *typedMeasurement) begin(value reflect.Value) (measurementVisit, error) {
	visit := measurementVisit{kind: value.Kind(), typeOf: value.Type(), pointer: value.Pointer()}
	if visit.pointer == 0 {
		return visit, nil
	}
	if _, exists := state.seen[visit]; exists {
		return visit, fmt.Errorf("typed JSON contains a cycle")
	}
	state.seen[visit] = struct{}{}
	return visit, nil
}

func (state *typedMeasurement) end(visit measurementVisit) {
	if visit.pointer != 0 {
		delete(state.seen, visit)
	}
}

func (state *typedMeasurement) value(value reflect.Value, depth int) (int, error) {
	if depth > maxDepth {
		return 0, fmt.Errorf("typed value depth exceeds %d", maxDepth)
	}
	if !value.IsValid() {
		return 4, nil
	}
	if value.Type() == rawMessageType {
		return state.rawMessage(value)
	}
	if value.Type() == jsonNumberType {
		return measureJSONNumber(value.String())
	}
	if hasCustomJSONMarshaler(value.Type()) {
		return 0, fmt.Errorf("custom JSON marshalers are unsupported")
	}
	switch value.Kind() {
	case reflect.Interface:
		if value.IsNil() {
			return 4, nil
		}
		return state.value(value.Elem(), depth)
	case reflect.Pointer:
		if value.IsNil() {
			return 4, nil
		}
		visit, err := state.begin(value)
		if err != nil {
			return 0, err
		}
		defer state.end(visit)
		return state.value(value.Elem(), depth)
	case reflect.Array, reflect.Slice:
		return state.sequence(value, depth)
	case reflect.Map:
		return state.objectMap(value, depth)
	case reflect.Struct:
		return state.objectStruct(value, depth)
	default:
		return measureScalar(value)
	}
}

func hasCustomJSONMarshaler(typeOf reflect.Type) bool {
	if typeOf.Implements(jsonMarshalerType) || typeOf.Implements(textMarshalerType) {
		return true
	}
	return typeOf.Kind() != reflect.Pointer &&
		(reflect.PointerTo(typeOf).Implements(jsonMarshalerType) ||
			reflect.PointerTo(typeOf).Implements(textMarshalerType))
}

func measureJSONNumber(value string) (int, error) {
	number, err := strconv.ParseInt(value, 10, 64)
	if err != nil || strconv.FormatInt(number, 10) != value {
		return 0, fmt.Errorf("number %q is not a canonical signed int64", value)
	}
	return len(value), nil
}

func measureScalar(value reflect.Value) (int, error) {
	switch value.Kind() {
	case reflect.String:
		return measureString(value.String())
	case reflect.Bool:
		if value.Bool() {
			return 4, nil
		}
		return 5, nil
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return len(strconv.FormatInt(value.Int(), 10)), nil
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		if value.Uint() > math.MaxInt64 {
			return 0, fmt.Errorf("typed JSON integer is outside signed int64")
		}
		return len(strconv.FormatUint(value.Uint(), 10)), nil
	case reflect.Float32, reflect.Float64, reflect.Complex64, reflect.Complex128:
		return 0, fmt.Errorf("typed JSON floating-point or complex values are unsupported")
	default:
		return 0, fmt.Errorf("unsupported typed JSON value %s", value.Kind())
	}
}

func measureString(value string) (int, error) {
	if err := validateString(value); err != nil {
		return 0, err
	}
	size := len(value) + 2
	for _, character := range value {
		if character == '"' || character == '\\' {
			size++
		}
	}
	return size, nil
}

func (state *typedMeasurement) sequence(value reflect.Value, depth int) (int, error) {
	if value.Kind() == reflect.Slice && value.Type().Elem().Kind() == reflect.Uint8 {
		return 0, fmt.Errorf("typed byte slices are unsupported JSON values")
	}
	if value.Kind() == reflect.Slice && value.IsNil() {
		return 4, nil
	}
	if value.Len() > maxArrayItems {
		return 0, fmt.Errorf("typed array item count exceeds %d", maxArrayItems)
	}
	if value.Len() == 0 {
		return 2, nil
	}
	var visit measurementVisit
	var err error
	if value.Kind() == reflect.Slice {
		visit, err = state.begin(value)
		if err != nil {
			return 0, err
		}
		defer state.end(visit)
	}
	total := 2
	for index := 0; index < value.Len(); index++ {
		child, childErr := state.value(value.Index(index), depth+1)
		if childErr != nil {
			return 0, childErr
		}
		if index > 0 {
			total, err = state.add(total, 1)
			if err != nil {
				return 0, err
			}
		}
		total, err = state.add(total, child)
		if err != nil {
			return 0, err
		}
	}
	return total, nil
}

func (state *typedMeasurement) objectMap(value reflect.Value, depth int) (int, error) {
	if value.IsNil() {
		return 4, nil
	}
	if value.Type().Key().Kind() != reflect.String || value.Len() > maxObjectFields {
		return 0, fmt.Errorf("typed JSON map must have at most %d string keys", maxObjectFields)
	}
	visit, err := state.begin(value)
	if err != nil {
		return 0, err
	}
	defer state.end(visit)
	keys := value.MapKeys()
	sort.Slice(keys, func(left, right int) bool { return keys[left].String() < keys[right].String() })
	total := 2
	for index, key := range keys {
		keySize, keyErr := measureObjectKey(key.String())
		if keyErr != nil {
			return 0, keyErr
		}
		child, childErr := state.value(value.MapIndex(key), depth+1)
		if childErr != nil {
			return 0, childErr
		}
		increment := keySize + 1 + child
		if index > 0 {
			increment++
		}
		total, err = state.add(total, increment)
		if err != nil {
			return 0, err
		}
	}
	return total, nil
}

func measureObjectKey(value string) (int, error) {
	if !asciiSnakeKey(value) {
		return 0, fmt.Errorf("object key %q is not ASCII snake_case", value)
	}
	return measureString(value)
}

func measuredStructFields(value reflect.Value) ([]measuredField, error) {
	fields := make([]measuredField, 0, value.NumField())
	typeOfValue := value.Type()
	for index := 0; index < value.NumField(); index++ {
		fieldType, fieldValue := typeOfValue.Field(index), value.Field(index)
		if fieldType.PkgPath != "" {
			continue
		}
		name, options, _ := strings.Cut(fieldType.Tag.Get("json"), ",")
		if name == "-" || hasJSONOption(options, "omitempty") && emptyJSONValue(fieldValue) ||
			hasJSONOption(options, "omitzero") && fieldValue.IsZero() {
			continue
		}
		if name == "" {
			if fieldType.Anonymous {
				return nil, fmt.Errorf("anonymous JSON struct fields are unsupported")
			}
			name = fieldType.Name
		}
		if hasJSONOption(options, "string") {
			return nil, fmt.Errorf("JSON string field option is unsupported")
		}
		fields = append(fields, measuredField{name: name, value: fieldValue})
	}
	if len(fields) > maxObjectFields {
		return nil, fmt.Errorf("typed object field count exceeds %d", maxObjectFields)
	}
	sort.Slice(fields, func(left, right int) bool { return fields[left].name < fields[right].name })
	return fields, nil
}

func (state *typedMeasurement) objectStruct(value reflect.Value, depth int) (int, error) {
	fields, err := measuredStructFields(value)
	if err != nil {
		return 0, err
	}
	total, prior := 2, ""
	for index, field := range fields {
		if field.name == prior {
			return 0, fmt.Errorf("duplicate typed JSON field %q", field.name)
		}
		prior = field.name
		keySize, keyErr := measureObjectKey(field.name)
		if keyErr != nil {
			return 0, keyErr
		}
		child, childErr := state.value(field.value, depth+1)
		if childErr != nil {
			return 0, childErr
		}
		increment := keySize + 1 + child
		if index > 0 {
			increment++
		}
		total, err = state.add(total, increment)
		if err != nil {
			return 0, err
		}
	}
	return total, nil
}

func hasJSONOption(options, target string) bool {
	for _, option := range strings.Split(options, ",") {
		if option == target {
			return true
		}
	}
	return false
}

func emptyJSONValue(value reflect.Value) bool {
	switch value.Kind() {
	case reflect.Array, reflect.Map, reflect.Slice, reflect.String:
		return value.Len() == 0
	case reflect.Bool, reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32,
		reflect.Int64, reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32,
		reflect.Uint64, reflect.Uintptr, reflect.Float32, reflect.Float64,
		reflect.Interface, reflect.Pointer:
		return value.IsZero()
	}
	return false
}

func (state *typedMeasurement) rawMessage(value reflect.Value) (int, error) {
	raw := value.Bytes()
	if len(raw) == 0 {
		if value.IsNil() {
			return 4, nil
		}
		return 0, fmt.Errorf("typed raw JSON must contain one value")
	}
	canonical, err := canonicalizeEncodedJSON(raw, state.maximum)
	if err != nil || !bytes.Equal(canonical, raw) {
		return 0, fmt.Errorf("typed raw JSON must be exact canonical JSON")
	}
	return len(raw), nil
}
