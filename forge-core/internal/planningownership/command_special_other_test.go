//go:build !linux && !darwin && !freebsd && !openbsd && !netbsd && !dragonfly

package planningownership

import "fmt"

func createSpecialTestFile(string) error { return fmt.Errorf("unsupported platform") }
