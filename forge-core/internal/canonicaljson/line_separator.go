// Package canonicaljson closes the one encoding/json/serde_json spelling
// difference admitted by Forge's exact UTF-8 contracts.
package canonicaljson

// UnescapeLineSeparators rewrites encoding/json's JSONP-safe U+2028 and U+2029
// escapes to the raw UTF-8 scalar spelling emitted by serde_json. The input
// must already be valid JSON produced by encoding/json; it is mutated in place.
func UnescapeLineSeparators(encoded []byte) []byte {
	write, precedingBackslashes := 0, 0
	for read := 0; read < len(encoded); {
		separator := read+5 < len(encoded) && encoded[read] == '\\' &&
			precedingBackslashes%2 == 0 && encoded[read+1] == 'u' &&
			encoded[read+2] == '2' && encoded[read+3] == '0' &&
			encoded[read+4] == '2' && (encoded[read+5] == '8' || encoded[read+5] == '9')
		if separator {
			encoded[write], encoded[write+1] = 0xe2, 0x80
			encoded[write+2] = 0xa8 + encoded[read+5] - '8'
			write, read, precedingBackslashes = write+3, read+6, 0
			continue
		}
		character := encoded[read]
		encoded[write] = character
		write, read = write+1, read+1
		if character == '\\' {
			precedingBackslashes++
		} else {
			precedingBackslashes = 0
		}
	}
	return encoded[:write]
}
