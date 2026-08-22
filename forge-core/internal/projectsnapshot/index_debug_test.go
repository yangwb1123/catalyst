package projectsnapshot

import (
	"strings"
	"testing"
)

func TestValidateTrackedIndexDebugExactSetAndZeroFlags(t *testing.T) {
	tracked := map[string]struct{}{"one.txt": {}, "nested/two.txt": {}}
	raw := indexDebugRecord("one.txt", "0") + indexDebugRecord("nested/two.txt", "0")
	if err := validateTrackedIndexDebug([]byte(raw), tracked); err != nil {
		t.Fatal(err)
	}
}

func TestValidateTrackedIndexDebugRejectsSetAndFlagDrift(t *testing.T) {
	tracked := map[string]struct{}{"one.txt": {}, "two.txt": {}}
	tests := map[string]string{
		"missing":       indexDebugRecord("one.txt", "0"),
		"outside":       indexDebugRecord("one.txt", "0") + indexDebugRecord("other.txt", "0"),
		"duplicate":     indexDebugRecord("one.txt", "0") + indexDebugRecord("one.txt", "0"),
		"intent-to-add": indexDebugRecord("one.txt", "20004000") + indexDebugRecord("two.txt", "0"),
		"skip-worktree": indexDebugRecord("one.txt", "40004000") + indexDebugRecord("two.txt", "0"),
	}
	for name, raw := range tests {
		t.Run(name, func(t *testing.T) {
			if err := validateTrackedIndexDebug([]byte(raw), tracked); err == nil {
				t.Fatal("invalid tracked index debug set was accepted")
			}
		})
	}
}

func TestValidateTrackedIndexDebugRejectsMalformedRecords(t *testing.T) {
	valid := indexDebugRecord("one.txt", "0")
	tests := map[string]string{
		"empty-path":        strings.Replace(valid, "one.txt\x00", "\x00", 1),
		"missing-nul":       strings.Replace(valid, "one.txt\x00", "one.txt", 1),
		"missing-block":     "one.txt\x00",
		"leading-space":     strings.Replace(valid, "  ctime:", "   ctime:", 1),
		"space-for-tab":     strings.Replace(valid, "\tino:", " ino:", 1),
		"extra-space":       strings.Replace(valid, "flags: 0", "flags:  0", 1),
		"leading-zero":      strings.Replace(valid, "ctime: 1:2", "ctime: 01:2", 1),
		"uppercase-flag":    strings.Replace(valid, "flags: 0", "flags: A", 1),
		"missing-final-lf":  strings.TrimSuffix(valid, "\n"),
		"extra-final-lf":    valid + "\n",
		"noncanonical-path": strings.Replace(valid, "one.txt\x00", "../one.txt\x00", 1),
	}
	tracked := map[string]struct{}{"one.txt": {}}
	for name, raw := range tests {
		t.Run(name, func(t *testing.T) {
			if err := validateTrackedIndexDebug([]byte(raw), tracked); err == nil {
				t.Fatal("malformed tracked index debug record was accepted")
			}
		})
	}
}

func indexDebugRecord(path, flags string) string {
	return path + "\x00" +
		"  ctime: 1:2\n" +
		"  mtime: 3:4\n" +
		"  dev: 5\tino: 6\n" +
		"  uid: 7\tgid: 8\n" +
		"  size: 9\tflags: " + flags + "\n"
}
