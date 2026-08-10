//go:build !unix

package gopackagedependencyobservationproducer

import "fmt"

func ensureSupportedPlatform() error {
	return fmt.Errorf("local Go package dependency observation requires a Unix platform")
}
