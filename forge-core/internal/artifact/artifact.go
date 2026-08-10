// Package artifact owns ForgeOS's append-only artifact provenance manifest.
// It captures content hashes for workflow-declared outputs and provides strict
// load, pure query, and current-file verification APIs. The package uses only
// the Go standard library.
package artifact

import (
	"bufio"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"forgeos/forge-core/internal/statefs"
)

const (
	// FormatV1 is the only manifest generation this binary writes.
	FormatV1 = "forgeos.artifact.v1"
	manifest = "artifacts.jsonl"
)

// Record is one immutable observation of a concrete workflow output.
type Record struct {
	Format       string `json:"_format,omitempty"`
	RunID        string `json:"run_id"`
	Workflow     string `json:"workflow"`
	Phase        string `json:"phase"`
	Agent        string `json:"agent"`
	Model        string `json:"model"`
	Path         string `json:"path"`
	SHA256       string `json:"sha256"`
	Size         int64  `json:"size"`
	CreatedAt    string `json:"created_at"`
	PromptSHA256 string `json:"prompt_sha256"`
}

// Metadata supplies run/build facts that are independent of artifact bytes.
type Metadata struct {
	RunID        string
	Workflow     string
	Phase        string
	Agent        string
	Model        string
	PromptSHA256 string
}

// Filter selects records by exact field match. Empty fields are wildcards.
type Filter struct {
	RunID    string
	Workflow string
	Phase    string
	Agent    string
	Model    string
	Path     string
}

// Store serializes append batches for one repository. One Store is shared by
// all parallel phases in a run so JSONL records cannot interleave.
type Store struct {
	root string
	mu   sync.Mutex
	Now  func() time.Time
}

// NewStore returns a repository-scoped manifest store.
func NewStore(root string) *Store {
	return &Store{root: root, Now: time.Now}
}

// ValidateFormat accepts pre-versioned records for backward compatibility and
// rejects every unknown non-empty generation.
func ValidateFormat(format string) error {
	if format == "" || format == FormatV1 {
		return nil
	}
	return fmt.Errorf("unsupported artifact format %q (supported: %s)", format, FormatV1)
}

// Digest returns the lowercase SHA-256 of data.
func Digest(data []byte) string {
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:])
}

// Capture reads one concrete repo-relative output and returns its provenance
// record. Lexical and symlink escapes fail closed.
func Capture(root, path string, meta Metadata) (Record, error) {
	full, normalized, err := containedFile(root, path)
	if err != nil {
		return Record{}, err
	}
	data, err := os.ReadFile(full)
	if err != nil {
		return Record{}, fmt.Errorf("artifact: read %q: %w", path, err)
	}
	if len(bytes.TrimSpace(data)) == 0 {
		return Record{}, fmt.Errorf("artifact: %q is empty", path)
	}
	rec := Record{
		Format: FormatV1, RunID: meta.RunID, Workflow: meta.Workflow,
		Phase: meta.Phase, Agent: meta.Agent, Model: meta.Model,
		Path: normalized, SHA256: Digest(data), Size: int64(len(data)),
		PromptSHA256: meta.PromptSHA256,
	}
	return rec, nil
}

// Append writes records as one serialized JSONL batch. It first validates the
// complete existing manifest, so an unknown generation or malformed line
// blocks new provenance rather than being silently extended.
func (s *Store) Append(records ...Record) error {
	if len(records) == 0 {
		return nil
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	path, err := manifestPath(s.root, true)
	if err != nil {
		return err
	}
	if _, err := loadFile(path); err != nil {
		return err
	}
	batch, err := s.encodeBatch(records)
	if err != nil {
		return err
	}
	return appendBatch(path, batch)
}

func (s *Store) encodeBatch(records []Record) ([]byte, error) {
	now := time.Now
	if s.Now != nil {
		now = s.Now
	}
	var batch bytes.Buffer
	for i := range records {
		rec := records[i]
		if rec.Format == "" {
			rec.Format = FormatV1
		}
		if rec.CreatedAt == "" {
			rec.CreatedAt = now().UTC().Format(time.RFC3339Nano)
		}
		if err := validateRecord(rec); err != nil {
			return nil, fmt.Errorf("artifact: record %d: %w", i+1, err)
		}
		line, err := json.Marshal(rec)
		if err != nil {
			return nil, fmt.Errorf("artifact: encode record %d: %w", i+1, err)
		}
		batch.Write(line)
		batch.WriteByte('\n')
	}
	return batch.Bytes(), nil
}

func appendBatch(path string, batch []byte) error {
	f, err := statefs.OpenRegular(path, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o600)
	if err != nil {
		return fmt.Errorf("artifact: open manifest: %w", err)
	}
	n, err := f.Write(batch)
	if err != nil {
		_ = f.Close()
		return fmt.Errorf("artifact: append manifest: %w", err)
	}
	if n != len(batch) {
		_ = f.Close()
		return fmt.Errorf("artifact: append manifest: %w", io.ErrShortWrite)
	}
	if err := f.Sync(); err != nil {
		_ = f.Close()
		return fmt.Errorf("artifact: sync manifest: %w", err)
	}
	if err := f.Close(); err != nil {
		return fmt.Errorf("artifact: close manifest: %w", err)
	}
	return nil
}

// Load reads the repository manifest. A missing file is an empty history;
// malformed lines and unsupported formats are explicit errors.
func Load(root string) ([]Record, error) {
	path, err := manifestPath(root, false)
	if err != nil {
		return nil, err
	}
	return loadFile(path)
}

func loadFile(path string) ([]Record, error) {
	data, found, err := statefs.ReadRegular(path, 64<<20)
	if err != nil {
		return nil, fmt.Errorf("artifact: open manifest: %w", err)
	}
	if !found {
		return nil, nil
	}
	return decodeLines(bytes.NewReader(data))
}

func decodeLines(r io.Reader) ([]Record, error) {
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 64*1024), 4<<20)
	var records []Record
	for line := 1; scanner.Scan(); line++ {
		var rec Record
		if err := json.Unmarshal(scanner.Bytes(), &rec); err != nil {
			return nil, fmt.Errorf("artifact: decode line %d: %w", line, err)
		}
		if err := validateRecord(rec); err != nil {
			return nil, fmt.Errorf("artifact: line %d: %w", line, err)
		}
		records = append(records, rec)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("artifact: read manifest: %w", err)
	}
	return records, nil
}

// Query is a pure, stable-order filter. It neither performs IO nor mutates the
// input slice.
func Query(records []Record, filter Filter) []Record {
	result := make([]Record, 0, len(records))
	for _, rec := range records {
		if matches(rec, filter) {
			result = append(result, rec)
		}
	}
	return result
}

func matches(rec Record, f Filter) bool {
	return (f.RunID == "" || rec.RunID == f.RunID) &&
		(f.Workflow == "" || rec.Workflow == f.Workflow) &&
		(f.Phase == "" || rec.Phase == f.Phase) &&
		(f.Agent == "" || rec.Agent == f.Agent) &&
		(f.Model == "" || rec.Model == f.Model) &&
		(f.Path == "" || rec.Path == f.Path)
}

// Verify compares a record with the file currently at its recorded path.
func Verify(root string, rec Record) error {
	if err := validateRecord(rec); err != nil {
		return fmt.Errorf("artifact: verify record: %w", err)
	}
	current, err := Capture(root, rec.Path, Metadata{
		RunID: rec.RunID, Workflow: rec.Workflow, Phase: rec.Phase,
		Agent: rec.Agent, Model: rec.Model, PromptSHA256: rec.PromptSHA256,
	})
	if err != nil {
		return err
	}
	if current.SHA256 != rec.SHA256 || current.Size != rec.Size {
		return fmt.Errorf("artifact: %q content mismatch: recorded sha256=%s size=%d, current sha256=%s size=%d",
			rec.Path, rec.SHA256, rec.Size, current.SHA256, current.Size)
	}
	return nil
}

func validateRecord(rec Record) error {
	if err := ValidateFormat(rec.Format); err != nil {
		return err
	}
	fields := map[string]string{
		"run_id": rec.RunID, "workflow": rec.Workflow, "phase": rec.Phase,
		"agent": rec.Agent, "model": rec.Model, "path": rec.Path,
		"sha256": rec.SHA256, "prompt_sha256": rec.PromptSHA256,
		"created_at": rec.CreatedAt,
	}
	for name, value := range fields {
		if strings.TrimSpace(value) == "" {
			return fmt.Errorf("%s is required", name)
		}
	}
	if rec.Size <= 0 {
		return fmt.Errorf("size must be positive")
	}
	if !validDigest(rec.SHA256) || !validDigest(rec.PromptSHA256) {
		return fmt.Errorf("sha256 fields must be 64 lowercase hexadecimal characters")
	}
	if _, err := time.Parse(time.RFC3339Nano, rec.CreatedAt); err != nil {
		return fmt.Errorf("created_at must be RFC3339: %w", err)
	}
	return validateRelative(rec.Path)
}

func validDigest(value string) bool {
	if len(value) != sha256.Size*2 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func validateRelative(path string) error {
	if path == "" || filepath.IsAbs(path) {
		return fmt.Errorf("path must be repo-relative")
	}
	clean := filepath.Clean(filepath.FromSlash(path))
	if clean == "." || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
		return fmt.Errorf("path escapes repository")
	}
	if filepath.ToSlash(clean) != path {
		return fmt.Errorf("path must be normalized")
	}
	return nil
}

func containedFile(root, path string) (string, string, error) {
	if err := validateRelative(filepath.ToSlash(path)); err != nil {
		return "", "", fmt.Errorf("artifact: path %q: %w", path, err)
	}
	rootReal, rootAbs, err := resolvedRoot(root)
	if err != nil {
		return "", "", err
	}
	normalized := filepath.ToSlash(filepath.Clean(filepath.FromSlash(path)))
	full := filepath.Join(rootAbs, filepath.FromSlash(normalized))
	resolved, err := filepath.EvalSymlinks(full)
	if err != nil {
		return "", "", fmt.Errorf("artifact: resolve %q: %w", path, err)
	}
	if err := ensureContained(rootReal, resolved); err != nil {
		return "", "", fmt.Errorf("artifact: path %q: %w", path, err)
	}
	info, err := os.Stat(resolved)
	if err != nil {
		return "", "", fmt.Errorf("artifact: stat %q: %w", path, err)
	}
	if !info.Mode().IsRegular() {
		return "", "", fmt.Errorf("artifact: %q is not a regular file", path)
	}
	return resolved, normalized, nil
}

func manifestPath(root string, create bool) (string, error) {
	_, rootAbs, err := resolvedRoot(root)
	if err != nil {
		return "", err
	}
	dir := filepath.Join(rootAbs, ".forge")
	if create {
		if err := statefs.EnsurePrivateDir(dir); err != nil {
			return "", fmt.Errorf("artifact: secure manifest directory: %w", err)
		}
	} else {
		info, err := os.Lstat(dir)
		if os.IsNotExist(err) {
			return filepath.Join(dir, manifest), nil
		}
		if err != nil || info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return "", fmt.Errorf("artifact: manifest directory must be real")
		}
	}
	path := filepath.Join(dir, manifest)
	if _, _, err := statefs.InspectRegular(path); err != nil {
		return "", fmt.Errorf("artifact: manifest path: %w", err)
	}
	return path, nil
}

func resolvedRoot(root string) (real, absolute string, err error) {
	if root == "" {
		root = "."
	}
	absolute, err = filepath.Abs(root)
	if err != nil {
		return "", "", fmt.Errorf("artifact: resolve repository root: %w", err)
	}
	real, err = filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", "", fmt.Errorf("artifact: resolve repository root: %w", err)
	}
	return real, absolute, nil
}

func ensureContained(root, target string) error {
	rel, err := filepath.Rel(root, target)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return fmt.Errorf("path escapes repository")
	}
	return nil
}
