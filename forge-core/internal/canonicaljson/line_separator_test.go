package canonicaljson

import "testing"

func TestUnescapeLineSeparatorsPreservesLiteralEscapes(t *testing.T) {
	cases := map[string]struct {
		encoded string
		want    string
	}{
		"raw scalars":       {`"x\u2028y\u2029z"`, "\"x\u2028y\u2029z\""},
		"literal spellings": {`"x\\u2028y\\u2029z"`, `"x\\u2028y\\u2029z"`},
		"slash then scalar": {`"x\\\u2028y"`, "\"x\\\\\u2028y\""},
		"unchanged":         {`{"value":"ordinary"}`, `{"value":"ordinary"}`},
	}
	for name, test := range cases {
		t.Run(name, func(t *testing.T) {
			got := string(UnescapeLineSeparators([]byte(test.encoded)))
			if got != test.want {
				t.Fatalf("normalized JSON = %q, want %q", got, test.want)
			}
		})
	}
}
