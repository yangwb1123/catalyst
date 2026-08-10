//go:build !unix

package evolvelocatorobservationproducer

import "fmt"

func ensureSupportedPlatform() error {
	return fmt.Errorf("local Evolve locator observation production v1 requires Unix semantics")
}
