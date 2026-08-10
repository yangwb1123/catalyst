package localcommandobservationproducer

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

const maxSymlinkHops = 32

// toolSnapshot resolves the command's top-level executable from the exact
// scrubbed PATH and binds the resolved regular executable bytes.
func toolSnapshot(ctx context.Context, command commandProfile, environment EnvironmentManifest) (ToolManifest, string, error) {
	if len(command.Argv) == 0 {
		return ToolManifest{}, "", fmt.Errorf("command argv is empty")
	}
	pathValue, ok := environmentValue(environment, "PATH")
	if !ok {
		return ToolManifest{}, "", fmt.Errorf("environment manifest lacks PATH")
	}
	resolved, err := resolveExecutable(ctx, command.Argv[0], pathValue)
	if err != nil {
		return ToolManifest{}, "", err
	}
	finalPath, hops, err := resolveTerminalSymlinks(ctx, resolved)
	if err != nil {
		return ToolManifest{}, "", err
	}
	manifest, err := inspectExecutable(ctx, command.Argv[0], resolved, finalPath, hops)
	if err != nil {
		return ToolManifest{}, "", err
	}
	_, digest, err := digestManifest(toolDigestDomain, manifest)
	return manifest, digest, err
}

func resolveExecutable(ctx context.Context, requested, pathValue string) (string, error) {
	if requested == "" || strings.ContainsRune(requested, filepath.Separator) {
		return "", fmt.Errorf("requested executable must be a basename")
	}
	directories, err := scrubbedPathDirectories(pathValue)
	if err != nil {
		return "", err
	}
	for _, directory := range directories {
		if err := ctx.Err(); err != nil {
			return "", fmt.Errorf("resolve executable: %w", err)
		}
		candidate := filepath.Join(directory, requested)
		info, err := os.Stat(candidate)
		if err == nil && info.Mode().IsRegular() && info.Mode().Perm()&0o111 != 0 {
			return candidate, nil
		}
		if err != nil && !os.IsNotExist(err) {
			return "", fmt.Errorf("inspect executable %q: %w", candidate, err)
		}
	}
	return "", fmt.Errorf("executable %q is not available on scrubbed PATH", requested)
}

func resolveTerminalSymlinks(ctx context.Context, path string) (string, []SymlinkHop, error) {
	current, pending, err := splitAbsolutePath(path)
	if err != nil {
		return "", nil, err
	}
	seen := make(map[string]struct{})
	hops := make([]SymlinkHop, 0)
	for len(pending) != 0 {
		if err := ctx.Err(); err != nil {
			return "", nil, fmt.Errorf("resolve executable symlinks: %w", err)
		}
		candidate := filepath.Join(current, pending[0])
		pending = pending[1:]
		info, err := os.Lstat(candidate)
		if err != nil {
			return "", nil, fmt.Errorf("inspect executable path %q: %w", candidate, err)
		}
		if info.Mode()&os.ModeSymlink == 0 {
			current = candidate
			continue
		}
		if len(hops) == maxSymlinkHops {
			return "", nil, fmt.Errorf("executable symlink depth exceeds %d", maxSymlinkHops)
		}
		if _, exists := seen[candidate]; exists {
			return "", nil, fmt.Errorf("executable symlink cycle at %q", candidate)
		}
		seen[candidate] = struct{}{}
		target, err := os.Readlink(candidate)
		if err != nil {
			return "", nil, fmt.Errorf("read executable symlink %q: %w", candidate, err)
		}
		hops = append(hops, SymlinkHop{Path: candidate, Target: target})
		if !filepath.IsAbs(target) {
			target = filepath.Join(filepath.Dir(candidate), target)
		}
		combined := filepath.Clean(target)
		for _, segment := range pending {
			combined = filepath.Join(combined, segment)
		}
		current, pending, err = splitAbsolutePath(combined)
		if err != nil {
			return "", nil, err
		}
	}
	return current, hops, nil
}

func splitAbsolutePath(path string) (string, []string, error) {
	clean := filepath.Clean(path)
	if !filepath.IsAbs(clean) {
		return "", nil, fmt.Errorf("executable path %q is not absolute", path)
	}
	volume := filepath.VolumeName(clean)
	remainder := strings.TrimPrefix(clean, volume)
	remainder = strings.TrimPrefix(remainder, string(filepath.Separator))
	root := volume + string(filepath.Separator)
	if remainder == "" {
		return root, nil, nil
	}
	return root, strings.Split(remainder, string(filepath.Separator)), nil
}

func inspectExecutable(ctx context.Context, requested, resolved, finalPath string, hops []SymlinkHop) (ToolManifest, error) {
	if err := validateToolText(requested, resolved, finalPath, hops); err != nil {
		return ToolManifest{}, err
	}
	info, err := os.Lstat(finalPath)
	if err != nil {
		return ToolManifest{}, fmt.Errorf("inspect final executable %q: %w", finalPath, err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0 {
		return ToolManifest{}, fmt.Errorf("final executable %q is not a regular executable", finalPath)
	}
	digest, bytesRead, err := hashFile(ctx, finalPath)
	if err != nil {
		return ToolManifest{}, err
	}
	if bytesRead != info.Size() {
		return ToolManifest{}, fmt.Errorf("executable %q changed while hashing", finalPath)
	}
	return ToolManifest{
		APIVersion: ToolAPIVersion, Bytes: bytesRead, Canonicalization: Canonicalization,
		FinalPath: finalPath, Mode: int64(info.Mode().Perm()), ProfileID: toolProfileID,
		RequestedPath: requested, ResolvedPath: resolved, SHA256: digest,
		SymlinkHops: append(make([]SymlinkHop, 0, len(hops)), hops...),
	}, nil
}

func validateToolText(requested, resolved, finalPath string, hops []SymlinkHop) error {
	for label, value := range map[string]string{
		"requested executable": requested, "resolved executable path": resolved,
		"final executable path": finalPath,
	} {
		if err := validateText(label, value, false); err != nil {
			return err
		}
	}
	for index, hop := range hops {
		if err := validateText(fmt.Sprintf("executable symlink_hops[%d].path", index), hop.Path, false); err != nil {
			return err
		}
		if err := validateText(fmt.Sprintf("executable symlink_hops[%d].target", index), hop.Target, false); err != nil {
			return err
		}
	}
	return nil
}

func hashFile(ctx context.Context, path string) (string, int64, error) {
	if err := ctx.Err(); err != nil {
		return "", 0, fmt.Errorf("hash %q: %w", path, err)
	}
	file, err := os.Open(path)
	if err != nil {
		return "", 0, fmt.Errorf("open %q for hashing: %w", path, err)
	}
	defer func() { _ = file.Close() }()
	before, err := file.Stat()
	if err != nil {
		return "", 0, fmt.Errorf("stat %q before hashing: %w", path, err)
	}
	if before.Size() > maxIndividualFileBytes {
		return "", 0, fmt.Errorf("file %q exceeds %d bytes", path, maxIndividualFileBytes)
	}
	digest, count, err := hashReaderWithLimit(ctx, path, file, maxIndividualFileBytes)
	if err != nil {
		return "", 0, err
	}
	after, err := file.Stat()
	if err != nil {
		return "", 0, fmt.Errorf("stat %q after hashing: %w", path, err)
	}
	if count != before.Size() || !os.SameFile(before, after) || before.Size() != after.Size() ||
		before.Mode() != after.Mode() || !before.ModTime().Equal(after.ModTime()) {
		return "", 0, fmt.Errorf("file %q changed while hashing", path)
	}
	return digest, count, nil
}

func hashReaderWithLimit(
	ctx context.Context,
	label string,
	reader io.Reader,
	limit int64,
) (string, int64, error) {
	if limit < 0 {
		return "", 0, fmt.Errorf("hash %q has invalid byte limit", label)
	}
	hasher, buffer := sha256.New(), make([]byte, 128<<10)
	var count int64
	for {
		if err := ctx.Err(); err != nil {
			return "", 0, fmt.Errorf("hash %q: %w", label, err)
		}
		read, readErr := reader.Read(buffer)
		if read > 0 {
			if int64(read) > limit-count {
				return "", 0, fmt.Errorf("file %q exceeds %d bytes", label, limit)
			}
			_, _ = hasher.Write(buffer[:read])
			count += int64(read)
		}
		if readErr == io.EOF {
			return hex.EncodeToString(hasher.Sum(nil)), count, nil
		}
		if readErr != nil {
			return "", 0, fmt.Errorf("hash %q: %w", label, readErr)
		}
	}
}
