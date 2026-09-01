// Package appserver owns the local Forge Workspace HTTP process boundary.
package appserver

import (
	"fmt"
	"net"
	"path/filepath"
	"strconv"
	"strings"
)

const (
	// APIVersion identifies the first read-only App Server surface.
	APIVersion = "forgeos.app-server/v1"
	// DefaultListenAddress is loopback-only by construction.
	DefaultListenAddress  = "127.0.0.1:7467"
	maxStateDirBytes      = 4096
	maxStateDirComponents = 64
)

// BuildInfo is the bounded public build identity exposed by health checks.
type BuildInfo struct {
	Version string
	Commit  string
}

// Config freezes the process-local server configuration before any state or
// listener is opened.
type Config struct {
	ListenAddress string
	StateDir      string
	Build         BuildInfo
}

// Validate rejects ambiguous state and non-loopback listener boundaries.
func (c Config) Validate() error {
	if err := validateStateDir(c.StateDir); err != nil {
		return err
	}
	if err := validateListenAddress(c.ListenAddress); err != nil {
		return err
	}
	return c.Build.validate()
}

func validateStateDir(path string) error {
	if path == "" {
		return fmt.Errorf("state-dir must be a nonempty absolute path")
	}
	if len(path) > maxStateDirBytes {
		return fmt.Errorf("state-dir exceeds %d bytes", maxStateDirBytes)
	}
	if statePathComponents(path) > maxStateDirComponents {
		return fmt.Errorf("state-dir exceeds %d components", maxStateDirComponents)
	}
	if !filepath.IsAbs(path) || strings.ContainsRune(path, 0) {
		return fmt.Errorf("state-dir must be a nonempty absolute path")
	}
	clean := filepath.Clean(path)
	if path != clean {
		return fmt.Errorf("state-dir must be a canonical absolute path")
	}
	if filepath.Dir(clean) == clean {
		return fmt.Errorf("state-dir must not be a filesystem root")
	}
	return nil
}

func statePathComponents(path string) int {
	components, inside := 0, false
	for index := 0; index < len(path); index++ {
		separator := path[index] == byte(filepath.Separator) ||
			filepath.Separator == '\\' && path[index] == '/'
		if separator {
			inside = false
		} else if !inside {
			components++
			inside = true
		}
	}
	return components
}

func validateListenAddress(address string) error {
	host, portText, err := net.SplitHostPort(address)
	if err != nil {
		return fmt.Errorf("listen address must be host:port: %w", err)
	}
	ip := net.ParseIP(host)
	if ip == nil || !ip.IsLoopback() {
		return fmt.Errorf("listen host must be a literal loopback IP")
	}
	port, err := strconv.Atoi(portText)
	if err != nil || port < 0 || port > 65535 {
		return fmt.Errorf("listen port must be an integer in 0..65535")
	}
	return nil
}

func (b BuildInfo) validate() error {
	if err := validateBuildValue("version", b.Version, true); err != nil {
		return err
	}
	return validateBuildValue("commit", b.Commit, false)
}

func validateBuildValue(name, value string, required bool) error {
	if required && value == "" {
		return fmt.Errorf("build %s must be nonempty", name)
	}
	if len(value) > 128 {
		return fmt.Errorf("build %s exceeds 128 bytes", name)
	}
	for _, character := range value {
		if !isBuildCharacter(character) {
			return fmt.Errorf("build %s contains an unsupported character", name)
		}
	}
	return nil
}

func isBuildCharacter(character rune) bool {
	return character >= 'a' && character <= 'z' ||
		character >= 'A' && character <= 'Z' ||
		character >= '0' && character <= '9' ||
		strings.ContainsRune(".+-_", character)
}
