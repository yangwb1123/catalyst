package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"forgeos/forge-core/internal/asset"
)

// releaseAgentBinding freezes the executable selected by the operator's explicit
// --release-agent-path.
// Release argv uses the canonical path rather than looking up "claude" again at
// spawn time, so a repository-local PATH shadow or later symlink swap cannot
// change which program crosses the docs-only delivery boundary.
type releaseAgentBinding struct {
	path           string
	expectedSHA256 string
	info           os.FileInfo
	err            error
}

func (b releaseAgentBinding) wrapArgv(argv []string) []string {
	if b.err != nil {
		return nil
	}
	return releasePinnedLauncherArgv(b.path, b.expectedSHA256, argv)
}

func releaseValidationPhase(stage string, phase asset.Phase) bool {
	switch stage {
	case "deploy":
		return phase.Agent == "release-engineer" && phase.Name == "release-plan-validation"
	case "rollback":
		return phase.Agent == "release-engineer" && phase.Name == "rollback-plan-validation"
	default:
		return false
	}
}

func trustedAgentOpts(o runOpts, phase asset.Phase, binding releaseAgentBinding) runOpts {
	if (phase.Agent == "release-engineer" || o.evolveProposalOnly) && binding.err == nil {
		o.agentCmd = binding.path
	}
	return o
}

func bindReleaseAgent(repoRoot, command, trustedPath, expectedSHA256 string) releaseAgentBinding {
	if strings.TrimSpace(command) != "claude" {
		return releaseAgentBinding{err: fmt.Errorf("trusted Claude execution requires the literal --agent-cmd=claude")}
	}
	if trustedPath == "" || !filepath.IsAbs(trustedPath) {
		return releaseAgentBinding{err: fmt.Errorf("--release-agent-path must be an explicit absolute path")}
	}
	if filepath.Base(filepath.Clean(trustedPath)) != "claude" {
		return releaseAgentBinding{err: fmt.Errorf("--release-agent-path must name an executable basename exactly %q", "claude")}
	}
	expectedSHA256 = strings.ToLower(strings.TrimSpace(expectedSHA256))
	if decoded, err := hex.DecodeString(expectedSHA256); err != nil || len(decoded) != sha256.Size {
		return releaseAgentBinding{err: fmt.Errorf("--release-agent-sha256 must be exactly 64 hexadecimal characters")}
	}
	found := filepath.Clean(trustedPath)
	if lexicalPathInsideRepo(repoRoot, found) {
		return releaseAgentBinding{err: fmt.Errorf("claude executable must not be supplied through the project repository")}
	}
	canonical, err := filepath.EvalSymlinks(found)
	if err != nil {
		return releaseAgentBinding{err: fmt.Errorf("resolve claude symlinks: %w", err)}
	}
	info, err := os.Stat(canonical)
	if err != nil || !info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0 {
		return releaseAgentBinding{err: fmt.Errorf("claude target is not a regular executable")}
	}
	if pathInsideRepo(repoRoot, canonical) {
		return releaseAgentBinding{err: fmt.Errorf("claude executable must not be supplied by the project repository")}
	}
	if err := validatePinnedInterpreter(canonical); err != nil {
		return releaseAgentBinding{err: err}
	}
	actualSHA256, err := fileSHA256(canonical)
	if err != nil {
		return releaseAgentBinding{err: fmt.Errorf("hash trusted claude executable: %w", err)}
	}
	if actualSHA256 != expectedSHA256 {
		return releaseAgentBinding{err: fmt.Errorf("trusted claude executable SHA-256 does not match --release-agent-sha256")}
	}
	if err := releasePinnedExecutionSupport(); err != nil {
		return releaseAgentBinding{err: err}
	}
	return releaseAgentBinding{
		path: canonical, expectedSHA256: expectedSHA256, info: info,
	}
}

// validateProposalExecutor gives proposal-only Evolve the same executable
// identity boundary used by release phases. A basename/PATH convention is not a
// trust anchor: the operator must pin an external Claude binary and its digest.
func validateProposalExecutor(p asset.Phase, o runOpts, isClaude bool, binding releaseAgentBinding) error {
	if !o.evolveProposalOnly {
		return nil
	}
	if !p.Readonly {
		return fmt.Errorf("proposal-only evolve phase %q must be readonly", p.Name)
	}
	if p.WritesADR != nil {
		return fmt.Errorf("proposal-only evolve phase %q forbids directory-scoped writes_adr", p.Name)
	}
	if !isClaude || strings.TrimSpace(o.agentCmd) != "claude" {
		return fmt.Errorf("proposal-only evolve requires Claude with literal --agent-cmd=claude; command %q is not trusted", o.agentCmd)
	}
	if binding.err != nil {
		return fmt.Errorf("proposal-only evolve requires a pinned Claude executable: %w", binding.err)
	}
	if err := binding.verify(); err != nil {
		return fmt.Errorf("proposal-only evolve phase %q: %w", p.Name, err)
	}
	if strings.TrimSpace(o.agentEnv) != "" {
		return fmt.Errorf("proposal-only evolve forbids --agent-env to prevent runtime injection")
	}
	tools := strings.TrimSpace(o.agentAllowedTools)
	if tools != "" && tools != defaultAgentAllowedTools {
		return fmt.Errorf("proposal-only evolve forbids custom --agent-allowed-tools")
	}
	return nil
}

func (b releaseAgentBinding) verify() error {
	if b.err != nil {
		return b.err
	}
	if err := releasePinnedExecutionSupport(); err != nil {
		return err
	}
	info, err := os.Stat(b.path)
	if err != nil {
		return fmt.Errorf("revalidate trusted claude executable: %w", err)
	}
	if b.info == nil || !os.SameFile(b.info, info) {
		return fmt.Errorf("trusted claude executable identity changed after binding")
	}
	if !info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0 {
		return fmt.Errorf("trusted claude target is no longer a regular executable")
	}
	if err := validatePinnedInterpreter(b.path); err != nil {
		return err
	}
	actualSHA256, err := fileSHA256(b.path)
	if err != nil {
		return fmt.Errorf("rehash trusted claude executable: %w", err)
	}
	if actualSHA256 != b.expectedSHA256 {
		return fmt.Errorf("trusted claude executable bytes changed after binding")
	}
	return nil
}

// validatePinnedInterpreter requires a Linux ELF image. Hashing a script or
// binfmt payload does not pin the interpreter that the kernel may open later,
// and that pathname has its own alias/TOCTOU surface.
func validatePinnedInterpreter(path string) error {
	file, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("inspect trusted claude executable header: %w", err)
	}
	defer func() { _ = file.Close() }()
	header := make([]byte, 4)
	n, err := file.Read(header)
	if err != nil && err != io.EOF {
		return fmt.Errorf("inspect trusted claude executable header: %w", err)
	}
	header = header[:n]
	if bytes.Equal(header, []byte{0x7f, 'E', 'L', 'F'}) {
		return nil
	}
	return fmt.Errorf("trusted claude executable must be a native Linux ELF; scripts and binfmt payloads are not pinned")
}

func fileSHA256(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer func() { _ = file.Close() }()
	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return "", err
	}
	return fmt.Sprintf("%x", hash.Sum(nil)), nil
}

func pathInsideRepo(repoRoot, candidate string) bool {
	root, err := filepath.Abs(repoRoot)
	if err != nil {
		return true
	}
	root, err = filepath.EvalSymlinks(root)
	if err != nil {
		return true
	}
	candidate, err = filepath.EvalSymlinks(candidate)
	if err != nil {
		return true
	}
	rel, err := filepath.Rel(root, candidate)
	return err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}

func lexicalPathInsideRepo(repoRoot, candidate string) bool {
	root, err := filepath.Abs(repoRoot)
	if err != nil {
		return true
	}
	candidate, err = filepath.Abs(candidate)
	if err != nil {
		return true
	}
	rel, err := filepath.Rel(filepath.Clean(root), filepath.Clean(candidate))
	return err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}

// validateReleaseExecutor closes command overrides, ambient integrations and
// filesystem aliases before CommandExecutor.Build can construct or spawn argv.
func validateReleaseExecutor(p asset.Phase, o runOpts, isClaude bool, binding releaseAgentBinding) error {
	if p.Agent != "release-engineer" {
		return nil
	}
	if !releaseApprovalStage(o.workflowStage) {
		return fmt.Errorf("release-engineer phase %q is only permitted in deploy/rollback workflows, not stage %q",
			p.Name, o.workflowStage)
	}
	if !p.Readonly {
		return fmt.Errorf("release-engineer phase %q must be readonly", p.Name)
	}
	if !isClaude || strings.TrimSpace(o.agentCmd) != "claude" {
		return fmt.Errorf("release-engineer phase %q only permits literal --agent-cmd=claude", p.Name)
	}
	if binding.err != nil {
		return fmt.Errorf("release-engineer phase %q: %w", p.Name, binding.err)
	}
	if err := binding.verify(); err != nil {
		return fmt.Errorf("release-engineer phase %q: %w", p.Name, err)
	}
	if strings.TrimSpace(o.agentEnv) != "" {
		return fmt.Errorf("release-engineer phase %q forbids --agent-env; deployment credentials must stay outside ForgeOS", p.Name)
	}
	tools := strings.TrimSpace(o.agentAllowedTools)
	if tools != "" && tools != defaultAgentAllowedTools {
		return fmt.Errorf("release-engineer phase %q forbids custom --agent-allowed-tools", p.Name)
	}
	switch strings.TrimSpace(o.agentPermission) {
	case "", "acceptEdits", "plan", "default":
	default:
		return fmt.Errorf("release-engineer phase %q has unsupported --agent-permission %q", p.Name, o.agentPermission)
	}
	if err := validateReleaseWriteRoot(o.root); err != nil {
		return fmt.Errorf("release-engineer phase %q: %w", p.Name, err)
	}
	patterns, err := releaseEmitPermissionPatterns(o.root, p)
	if err != nil {
		return fmt.Errorf("release-engineer phase %q: %w", p.Name, err)
	}
	if len(patterns) == 0 {
		return fmt.Errorf("release-engineer phase %q must declare at least one exact emit", p.Name)
	}
	return nil
}

// releaseEmitPermissionPatterns converts only the phase's declared emits into
// exact Claude Edit permission rules. Globs and directory grants are rejected:
// a release phase may write no path merely because it shares docs/release.
func releaseEmitPermissionPatterns(repoRoot string, phase asset.Phase) ([]string, error) {
	seen := make(map[string]bool, len(phase.Emits))
	patterns := make([]string, 0, len(phase.Emits))
	for _, emit := range phase.Emits {
		_, normalized, err := containedRepoPath(repoRoot, emit)
		if err != nil {
			return nil, fmt.Errorf("declared emit %q: %w", emit, err)
		}
		relative := filepath.ToSlash(normalized)
		if emit != relative || !strings.HasPrefix(relative, "docs/release/") ||
			!safeADRRelativePath(relative) {
			return nil, fmt.Errorf("declared emit %q must be an exact normalized file under docs/release", emit)
		}
		if seen[relative] {
			continue
		}
		seen[relative] = true
		patterns = append(patterns, "/"+relative)
	}
	return patterns, nil
}

func validateReleaseWriteRoot(repoRoot string) error {
	target, relative, err := containedRepoPath(repoRoot, filepath.Join("docs", "release"))
	if err != nil {
		return fmt.Errorf("invalid docs/release write root: %w", err)
	}
	if filepath.ToSlash(relative) != "docs/release" {
		return fmt.Errorf("invalid docs/release write root %q", relative)
	}
	root, err := filepath.Abs(repoRoot)
	if err != nil {
		return fmt.Errorf("resolve repository root: %w", err)
	}
	for _, path := range []string{root, filepath.Join(root, "docs"), target} {
		info, statErr := os.Lstat(path)
		if statErr != nil {
			return fmt.Errorf("release write-root component %q: %w", path, statErr)
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return fmt.Errorf("release write-root component %q must be a real directory", path)
		}
	}
	return filepath.WalkDir(target, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("release write root contains symlink %q", path)
		}
		if info.IsDir() {
			return nil
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("release write root contains non-regular entry %q", path)
		}
		if !releaseRegularSingleLink(info) {
			return fmt.Errorf("release write root contains hard-linked or unverifiable file %q", path)
		}
		return nil
	})
}

// snapshotReleaseTree freezes every release-tree entry, including directories
// and permission bits. Declared emits may change; every other delta is rejected
// after the agent returns.
type releaseTreeEntry struct {
	kind       string
	mode       os.FileMode
	size       int64
	modifiedNS int64
	sha256     string
	identity   os.FileInfo
}

type releaseTreeSnapshot map[string]releaseTreeEntry

func snapshotReleaseTree(repoRoot string) (releaseTreeSnapshot, error) {
	if err := validateReleaseWriteRoot(repoRoot); err != nil {
		return nil, err
	}
	target, _, err := containedRepoPath(repoRoot, filepath.Join("docs", "release"))
	if err != nil {
		return nil, err
	}
	root, err := filepath.Abs(repoRoot)
	if err != nil {
		return nil, fmt.Errorf("resolve repository root: %w", err)
	}
	snapshot := make(releaseTreeSnapshot)
	err = filepath.WalkDir(target, func(path string, entry os.DirEntry, walkErr error) error {
		relative, state, err := snapshotReleaseTreeEntry(repoRoot, root, path, entry, walkErr)
		if err == nil {
			snapshot[relative] = state
		}
		return err
	})
	if err != nil {
		return nil, err
	}
	return snapshot, nil
}

func snapshotReleaseTreeEntry(repoRoot, root, path string, entry os.DirEntry, walkErr error) (string, releaseTreeEntry, error) {
	if walkErr != nil {
		return "", releaseTreeEntry{}, walkErr
	}
	info, err := entry.Info()
	if err != nil {
		return "", releaseTreeEntry{}, err
	}
	relative, err := filepath.Rel(root, path)
	if err != nil {
		return "", releaseTreeEntry{}, err
	}
	relative = filepath.ToSlash(relative)
	if info.Mode()&os.ModeSymlink != 0 {
		return "", releaseTreeEntry{}, fmt.Errorf("release write root contains symlink %q", path)
	}
	if info.IsDir() {
		return relative, releaseTreeEntry{
			kind: "directory", mode: info.Mode(), identity: info,
		}, nil
	}
	if !info.Mode().IsRegular() || !releaseRegularSingleLink(info) {
		return "", releaseTreeEntry{}, fmt.Errorf("release write root contains non-regular or aliased entry %q", path)
	}
	data, present, err := readReleaseFileBytes(repoRoot, relative)
	if err != nil {
		return "", releaseTreeEntry{}, err
	}
	if !present {
		return "", releaseTreeEntry{}, fmt.Errorf("release file %q disappeared while snapshotting", relative)
	}
	after, err := os.Lstat(path)
	if err != nil || !os.SameFile(info, after) {
		return "", releaseTreeEntry{}, fmt.Errorf("release file %q changed while snapshotting", relative)
	}
	return relative, releaseTreeEntry{
		kind: "regular", mode: after.Mode(), size: after.Size(),
		modifiedNS: after.ModTime().UnixNano(),
		sha256:     fmt.Sprintf("%x", sha256.Sum256(data)),
		identity:   after,
	}, nil
}

func validateReleaseTreeDelta(repoRoot string, before releaseTreeSnapshot, emits []string) error {
	if before == nil {
		return fmt.Errorf("missing frozen release-tree snapshot")
	}
	phase := asset.Phase{Agent: "release-engineer", Emits: emits}
	patterns, err := releaseEmitPermissionPatterns(repoRoot, phase)
	if err != nil {
		return err
	}
	allowed := make(map[string]bool, len(patterns))
	for _, pattern := range patterns {
		allowed[strings.TrimPrefix(pattern, "/")] = true
	}
	after, err := snapshotReleaseTree(repoRoot)
	if err != nil {
		return err
	}
	union := make(map[string]bool, len(before)+len(after))
	for path := range before {
		union[path] = true
	}
	for path := range after {
		union[path] = true
	}
	paths := make([]string, 0, len(union))
	for path := range union {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	for _, path := range paths {
		beforeEntry, existedBefore := before[path]
		afterEntry, existsAfter := after[path]
		unchanged := existedBefore == existsAfter
		if unchanged && existedBefore {
			unchanged = sameReleaseTreeEntry(beforeEntry, afterEntry)
		}
		if !unchanged && !allowed[path] {
			return fmt.Errorf("undeclared release path %q changed during this attempt", path)
		}
	}
	return nil
}

func sameReleaseTreeEntry(left, right releaseTreeEntry) bool {
	return left.kind == right.kind &&
		left.mode == right.mode &&
		left.size == right.size &&
		left.modifiedNS == right.modifiedNS &&
		left.sha256 == right.sha256 &&
		left.identity != nil && right.identity != nil &&
		os.SameFile(left.identity, right.identity)
}
