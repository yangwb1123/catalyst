//go:build !unix

package gopackagedependencyobservationproducer

import (
	"context"
	"testing"
)

func TestUnsupportedPlatformFailsClosed(t *testing.T) {
	if err := ensureSupportedPlatform(); err == nil {
		t.Fatal("non-Unix platform accepted")
	}
	production, err := Produce(context.Background(), "/unobserved", ".", "run-non-unix")
	if err == nil || production != nil {
		t.Fatalf("non-Unix Produce returned production=%v error=%v", production, err)
	}
}
