package localcommandobservationproducer

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestHashReaderWithLimitStopsAtTheCumulativeBoundary(t *testing.T) {
	digest, count, err := hashReaderWithLimit(
		context.Background(), "fixture", bytes.NewReader([]byte("1234")), 4,
	)
	if err != nil || count != 4 || digest != sha256Bytes([]byte("1234")) {
		t.Fatalf("exact-limit hash = %q, %d, %v", digest, count, err)
	}
	if _, _, err := hashReaderWithLimit(
		context.Background(), "fixture", bytes.NewReader([]byte("12345")), 4,
	); err == nil || !strings.Contains(err.Error(), "exceeds 4 bytes") {
		t.Fatalf("over-limit reader error = %v", err)
	}
}

func TestToolSnapshotBindsResolvedExecutableAndSymlink(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "node-real")
	if err := os.WriteFile(target, []byte("executable-v1"), 0o755); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(root, "node")
	if err := os.Symlink(filepath.Base(target), link); err != nil {
		t.Fatal(err)
	}
	environment, _, _, err := environmentSnapshot([]string{"PATH=" + root})
	if err != nil {
		t.Fatal(err)
	}
	command, _ := commandForClass(CommandGate)
	manifest, digest, err := toolSnapshot(context.Background(), command, environment)
	if err != nil {
		t.Fatal(err)
	}
	if manifest.RequestedPath != "node" || manifest.ResolvedPath != link ||
		manifest.FinalPath != target || manifest.Bytes != int64(len("executable-v1")) ||
		manifest.SHA256 != sha256Bytes([]byte("executable-v1")) || len(manifest.SymlinkHops) != 1 || len(digest) != 64 {
		t.Fatalf("unexpected tool manifest %#v digest=%q", manifest, digest)
	}
	if err := os.WriteFile(target, []byte("executable-v2"), 0o755); err != nil {
		t.Fatal(err)
	}
	_, changed, err := toolSnapshot(context.Background(), command, environment)
	if err != nil || changed == digest {
		t.Fatalf("tool byte drift not reflected: digest=%s changed=%s err=%v", digest, changed, err)
	}
}

func TestValidateToolTextRejectsNoncanonicalFilesystemStrings(t *testing.T) {
	for _, test := range []struct {
		name  string
		value string
	}{
		{name: "invalid utf8", value: string([]byte{0xff})},
		{name: "control", value: "bad\npath"},
		{name: "bidi", value: "bad\u202epath"},
		{name: "oversized", value: strings.Repeat("x", maxTextBytes+1)},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := validateToolText("node", "/bin/node", "/bin/node", []SymlinkHop{
				{Path: "/bin/node", Target: test.value},
			})
			if err == nil {
				t.Fatalf("noncanonical symlink target %q was accepted", test.name)
			}
		})
	}
}

func TestResolveExecutableValidatesEntireScrubbedPathBeforeSearch(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "node"), []byte("node"), 0o755); err != nil {
		t.Fatal(err)
	}
	separator := string(filepath.ListSeparator)
	for _, test := range []struct {
		name      string
		pathValue string
	}{
		{name: "later relative", pathValue: root + separator + "relative"},
		{name: "later empty", pathValue: root + separator},
		{name: "later nonnormalized", pathValue: root + separator + root + string(filepath.Separator) + "."},
	} {
		t.Run(test.name, func(t *testing.T) {
			if _, err := resolveExecutable(context.Background(), "node", test.pathValue); err == nil ||
				!strings.Contains(err.Error(), "normalized absolute path") {
				t.Fatalf("malformed complete PATH %q error = %v", test.pathValue, err)
			}
		})
	}
}

func TestToolSnapshotRejectsMissingExecutable(t *testing.T) {
	command, _ := commandForClass(CommandGate)
	pathValue := t.TempDir()
	environment, _, _, err := environmentSnapshot([]string{"PATH=" + pathValue})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := toolSnapshot(context.Background(), command, environment); err == nil {
		t.Fatalf("missing executable on PATH %q accepted", pathValue)
	}
}

func TestToolSnapshotResolvesSymlinkedPathDirectory(t *testing.T) {
	root := t.TempDir()
	realBin := filepath.Join(root, "real-bin")
	if err := os.Mkdir(realBin, 0o755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(realBin, "node")
	if err := os.WriteFile(target, []byte("node"), 0o755); err != nil {
		t.Fatal(err)
	}
	linkedBin := filepath.Join(root, "bin")
	if err := os.Symlink(filepath.Base(realBin), linkedBin); err != nil {
		t.Fatal(err)
	}
	environment, _, _, err := environmentSnapshot([]string{"PATH=" + linkedBin})
	if err != nil {
		t.Fatal(err)
	}
	command, _ := commandForClass(CommandGate)
	manifest, _, err := toolSnapshot(context.Background(), command, environment)
	if err != nil {
		t.Fatal(err)
	}
	if manifest.FinalPath != target || len(manifest.SymlinkHops) != 1 ||
		manifest.SymlinkHops[0].Path != linkedBin {
		t.Fatalf("parent symlink was not fully bound: %#v", manifest)
	}
}
