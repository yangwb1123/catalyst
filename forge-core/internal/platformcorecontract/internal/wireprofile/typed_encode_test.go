package wireprofile

import (
	"reflect"
	"strconv"
	"strings"
	"testing"
)

type typedEncodingFixture struct {
	Items []any  `json:"items"`
	Text  string `json:"text"`
}

func TestTypedCanonicalEncodesWithoutAnIntermediateValueTree(t *testing.T) {
	value := typedEncodingFixture{
		Items: []any{int64(1), "x"},
		Text:  "<&>",
	}
	encoded, err := TypedCanonical(&value, 128)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := string(encoded), `{"items":[1,"x"],"text":"<&>"}`; got != want {
		t.Fatalf("canonical = %s, want %s", got, want)
	}
}

func TestTypedCanonicalRejectsOversizedCollectionsBeforeTraversal(t *testing.T) {
	value := typedEncodingFixture{Items: make([]any, maxArrayItems+1), Text: "x"}
	if _, err := TypedCanonical(&value, 1<<20); err == nil ||
		!strings.Contains(err.Error(), "array exceeds") {
		t.Fatalf("oversized typed array error = %v", err)
	}
}

func TestTextLengthPrecedesUTF8Traversal(t *testing.T) {
	value := strings.Repeat("x", maxStringBytes+1) + "\xff"
	if err := ValidateText(value, "fixture", maxStringBytes, false); err == nil ||
		!strings.Contains(err.Error(), "within") {
		t.Fatalf("oversized invalid text error = %v", err)
	}
}

func TestTypedCanonicalEnforcesTextProfile(t *testing.T) {
	tests := []struct {
		name  string
		value any
	}{
		{name: "invalid UTF-8 string", value: string([]byte{0xff})},
		{name: "forbidden scalar string", value: "unsafe\x00text"},
		{name: "oversized string", value: strings.Repeat("x", maxStringBytes+1)},
		{name: "invalid map key", value: map[string]any{"unsafe\x00key": true}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := TypedCanonical(test.value, 1<<20); err == nil {
				t.Fatal("profile-invalid typed text was accepted")
			}
		})
	}
}

func TestTypedCanonicalRejectsStructFieldCountBeforeCollection(t *testing.T) {
	fields := make([]reflect.StructField, maxObjectFields+1)
	for index := range fields {
		fields[index] = reflect.StructField{
			Name: "Field" + strconv.Itoa(index),
			Type: reflect.TypeOf(""),
			Tag:  reflect.StructTag(`json:"field_` + strconv.Itoa(index) + `"`),
		}
	}
	value := reflect.New(reflect.StructOf(fields)).Elem().Interface()
	if _, err := TypedCanonical(value, 1<<20); err == nil ||
		!strings.Contains(err.Error(), "object exceeds") {
		t.Fatalf("oversized typed struct error = %v", err)
	}
}
