// Package declaredartifact binds an explicit set of product-source files to
// their exact, stable-read content digests. It observes files; it does not
// write artifacts or claim that the repository is an atomic snapshot.
package declaredartifact

import (
	"context"
	"fmt"
	"path"
	"strings"
	"unicode/utf8"

	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/productsource"
)

const (
	maxFiles      = 4_096
	maxFileBytes  = int64(64 << 20)
	maxTotalBytes = int64(512 << 20)
	maxPathDepth  = 128
)

type regularFileReader func(
	context.Context,
	productsource.Snapshot,
	[]string,
	productsource.RegularReadLimits,
) ([]productsource.RegularFile, error)

// Capture returns a manifest for the exact bytes of the declared regular
// files. Paths are repository-relative and must already be canonical, sorted,
// and unique. A nil or empty declaration is normalized to a canonical empty
// manifest after the captured repository root is revalidated.
func Capture(
	ctx context.Context,
	snapshot productsource.Snapshot,
	paths []string,
) (outputbinding.ArtifactManifest, error) {
	return captureWithReader(ctx, snapshot, paths, productsource.ReadSingleLinkDeclaredFiles)
}

func captureWithReader(
	ctx context.Context,
	snapshot productsource.Snapshot,
	paths []string,
	read regularFileReader,
) (outputbinding.ArtifactManifest, error) {
	declared := append([]string{}, paths...)
	if err := validateDeclaredPaths(declared); err != nil {
		return outputbinding.ArtifactManifest{}, err
	}
	if err := productsource.ValidateDeclaredRegularFileSet(snapshot, declared); err != nil {
		return outputbinding.ArtifactManifest{}, err
	}
	files, err := read(ctx, snapshot, declared, regularReadLimits())
	if err != nil {
		return outputbinding.ArtifactManifest{}, fmt.Errorf("capture declared artifacts: %w", err)
	}
	items, err := bindRegularFiles(declared, files)
	if err != nil {
		return outputbinding.ArtifactManifest{}, err
	}
	manifest, err := outputbinding.SealManifest(items)
	if err != nil {
		return outputbinding.ArtifactManifest{}, fmt.Errorf("seal declared artifacts: %w", err)
	}
	return manifest, nil
}

func validateDeclaredPaths(paths []string) error {
	if len(paths) > maxFiles {
		return fmt.Errorf("declared artifacts exceed %d files", maxFiles)
	}
	for index, value := range paths {
		if err := validateDeclaredPath(value); err != nil {
			return fmt.Errorf("declared artifact path %d: %w", index, err)
		}
		if index > 0 && paths[index-1] >= value {
			return fmt.Errorf("declared artifact paths must be strictly sorted and unique")
		}
	}
	return nil
}

func validateDeclaredPath(value string) error {
	hasDrive := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if !utf8.ValidString(value) {
		return fmt.Errorf("path must be valid UTF-8")
	}
	if value == "" || value == "." || hasDrive || strings.Contains(value, `\`) ||
		strings.HasPrefix(value, "/") || strings.HasSuffix(value, "/") ||
		strings.Contains(value, "//") || path.Clean(value) != value {
		return fmt.Errorf("path must be canonical repository-relative forward-slash form")
	}
	segments := strings.Split(value, "/")
	for _, segment := range segments {
		if segment == "." || segment == ".." {
			return fmt.Errorf("path contains an unsafe segment")
		}
	}
	if len(segments) > maxPathDepth {
		return fmt.Errorf("path exceeds %d components", maxPathDepth)
	}
	return nil
}

func bindRegularFiles(
	paths []string,
	files []productsource.RegularFile,
) ([]outputbinding.ManifestItem, error) {
	if len(files) != len(paths) {
		return nil, fmt.Errorf("declared artifact reader returned a mismatched file set")
	}
	items := make([]outputbinding.ManifestItem, len(files))
	var total int64
	for index, file := range files {
		if file.Path != paths[index] {
			return nil, fmt.Errorf("declared artifact reader returned path %q for %q", file.Path, paths[index])
		}
		size := int64(len(file.Content))
		var err error
		total, err = addFileBytes(total, size)
		if err != nil {
			return nil, fmt.Errorf("declared artifact %q: %w", file.Path, err)
		}
		digest := outputbinding.SHA256(file.Content)
		if digest != file.SHA256 {
			return nil, fmt.Errorf("declared artifact %q digest differs from stable read", file.Path)
		}
		items[index] = outputbinding.ManifestItem{Bytes: size, Path: file.Path, SHA256: digest}
	}
	return items, nil
}

func addFileBytes(total, size int64) (int64, error) {
	if size < 0 || size > maxFileBytes {
		return 0, fmt.Errorf("file exceeds %d bytes", maxFileBytes)
	}
	if total < 0 || total > maxTotalBytes-size {
		return 0, fmt.Errorf("files exceed %d total bytes", maxTotalBytes)
	}
	return total + size, nil
}

func asciiLetter(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func regularReadLimits() productsource.RegularReadLimits {
	return productsource.RegularReadLimits{
		MaxFiles:      maxFiles,
		MaxFileBytes:  maxFileBytes,
		MaxTotalBytes: maxTotalBytes,
		MaxPathDepth:  maxPathDepth,
	}
}
