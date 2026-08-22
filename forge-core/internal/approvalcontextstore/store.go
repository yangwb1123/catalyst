// Package approvalcontextstore persists strict repository-local approval
// contexts. The bytes are local observations, not authenticated approval.
package approvalcontextstore

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/approvalcontext"
	"forgeos/forge-core/internal/statefs"
)

const maxContextBytes = int64(64 << 10)

func Path(root, stage string) (string, error) {
	if root == "" {
		return "", fmt.Errorf("approval context store: repository root is required")
	}
	if !boundStage(stage) {
		return "", fmt.Errorf("approval context store: unsupported stage %q", stage)
	}
	directory := filepath.Join(root, ".forge")
	path := filepath.Join(directory, stage+".approval-context.json")
	relative, err := filepath.Rel(directory, path)
	if err != nil || filepath.IsAbs(relative) || relative != stage+".approval-context.json" ||
		strings.Contains(relative, string(filepath.Separator)+"..") {
		return "", fmt.Errorf("approval context store: path for stage %q escapes .forge", stage)
	}
	return path, nil
}

// Write atomically publishes exact compact canonical bytes without a trailing
// LF and returns the detached context digest.
func Write(root string, value approvalcontext.Context) (string, error) {
	path, err := Path(root, value.Stage)
	if err != nil {
		return "", err
	}
	data, err := approvalcontext.CanonicalContextJSON(value)
	if err != nil {
		return "", fmt.Errorf("approval context store: encode: %w", err)
	}
	digest, err := approvalcontext.ContextSHA256(value)
	if err != nil {
		return "", fmt.Errorf("approval context store: digest: %w", err)
	}
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		return "", fmt.Errorf("approval context store: secure directory: %w", err)
	}
	if err := statefs.AtomicWrite(path, data, 0o600); err != nil {
		return "", fmt.Errorf("approval context store: write: %w", err)
	}
	return digest, nil
}

// Load performs a side-effect-free strict private/single-link read and returns
// both the decoded context and its detached digest.
func Load(root, stage string) (approvalcontext.Context, string, error) {
	path, err := Path(root, stage)
	if err != nil {
		return approvalcontext.Context{}, "", err
	}
	if err := validatePrivateState(path); err != nil {
		return approvalcontext.Context{}, "", err
	}
	data, present, err := statefs.ReadRegularUnmodified(path, maxContextBytes)
	if err != nil || !present {
		if err == nil {
			err = os.ErrNotExist
		}
		return approvalcontext.Context{}, "", fmt.Errorf("approval context store: read: %w", err)
	}
	value, err := approvalcontext.DecodeCanonicalContext(data)
	if err != nil {
		return approvalcontext.Context{}, "", fmt.Errorf("approval context store: decode: %w", err)
	}
	if value.Stage != stage || value.Workflow != stage {
		return approvalcontext.Context{}, "", fmt.Errorf("approval context store: context identity does not match path")
	}
	digest, err := approvalcontext.ContextSHA256(value)
	if err != nil {
		return approvalcontext.Context{}, "", fmt.Errorf("approval context store: digest: %w", err)
	}
	return value, digest, nil
}

func validatePrivateState(path string) error {
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil || !present {
		if err == nil {
			err = os.ErrNotExist
		}
		return fmt.Errorf("approval context store: inspect directory: %w", err)
	}
	if directory.Mode().Perm() != 0o700 {
		return fmt.Errorf("approval context store: .forge directory must have mode 0700")
	}
	file, present, err := statefs.InspectRegular(path)
	if err != nil || !present {
		if err == nil {
			err = os.ErrNotExist
		}
		return fmt.Errorf("approval context store: inspect context: %w", err)
	}
	if file.Mode().Perm() != 0o600 {
		return fmt.Errorf("approval context store: context must have mode 0600")
	}
	return nil
}

func boundStage(stage string) bool {
	return stage == "design" || stage == "deploy" || stage == "rollback"
}
