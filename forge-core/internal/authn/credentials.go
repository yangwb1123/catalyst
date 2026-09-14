package authn

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"unicode"
)

const maxCredentialBytes = 4096

func validateCredentialFile(path string) error {
	if path == "" || !filepath.IsAbs(path) || filepath.Clean(path) != path {
		return fmt.Errorf("authn: introspection secret file must be a canonical absolute path")
	}
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 || info.Mode().Perm()&0o077 != 0 {
		return fmt.Errorf("authn: introspection secret file must be regular and private to its owner")
	}
	return nil
}

func loadCredentialFile(path string) (string, error) {
	if err := validateCredentialFile(path); err != nil {
		return "", err
	}
	before, err := os.Lstat(path)
	if err != nil {
		return "", fmt.Errorf("authn: inspect introspection secret file: %w", err)
	}
	file, err := os.Open(path)
	if err != nil {
		return "", fmt.Errorf("authn: open introspection secret file: %w", err)
	}
	defer file.Close()
	after, err := file.Stat()
	if err != nil || !after.Mode().IsRegular() || after.Mode().Perm()&0o077 != 0 || !os.SameFile(before, after) {
		return "", fmt.Errorf("authn: introspection secret file changed while opening")
	}
	secret, err := io.ReadAll(io.LimitReader(file, maxCredentialBytes+1))
	if err != nil || len(secret) > maxCredentialBytes {
		clear(secret)
		return "", fmt.Errorf("authn: introspection secret file is unreadable or too large")
	}
	if len(secret) > 0 && secret[len(secret)-1] == '\n' {
		secret = secret[:len(secret)-1]
		if len(secret) > 0 && secret[len(secret)-1] == '\r' {
			secret = secret[:len(secret)-1]
		}
	}
	if len(secret) == 0 || strings.ContainsFunc(string(secret), unicode.IsControl) {
		clear(secret)
		return "", fmt.Errorf("authn: introspection secret file must contain one nonempty credential line")
	}
	value := string(secret)
	clear(secret)
	return value, nil
}
