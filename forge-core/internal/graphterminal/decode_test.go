package graphterminal

import (
	"bytes"
	"testing"
)

type repeatedByteReader byte

func (reader repeatedByteReader) Read(buffer []byte) (int, error) {
	for index := range buffer {
		buffer[index] = byte(reader)
	}
	return len(buffer), nil
}

func TestDecodeControlRejectsInvalidUTF8AndOversize(t *testing.T) {
	if _, err := DecodeControl(bytes.NewReader([]byte{'{', '"', 0xff, '"', '}'})); err == nil {
		t.Fatal("accepted invalid UTF-8")
	}
	if _, err := DecodeControl(repeatedByteReader(' ')); err == nil {
		t.Fatal("accepted terminal control larger than the protocol bound")
	}
}
