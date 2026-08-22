package projectsnapshot

import (
	"context"
	"strings"
	"testing"
)

func TestParseTrackedRejectsDuplicateMalformedAndUnmergedRecords(t *testing.T) {
	valid := trackedInventoryItem("100644", strings.Repeat("1", 40), "0", "file.txt")
	tests := map[string][]byte{
		"duplicate":        append(append([]byte{}, valid...), valid...),
		"missing-nul":      valid[:len(valid)-1],
		"missing-tab":      []byte("100644 " + strings.Repeat("1", 40) + " 0 file.txt\x00"),
		"double-space":     []byte("100644  " + strings.Repeat("1", 40) + " 0\tfile.txt\x00"),
		"leading-space":    []byte(" 100644 " + strings.Repeat("1", 40) + " 0\tfile.txt\x00"),
		"trailing-space":   []byte("100644 " + strings.Repeat("1", 40) + " 0 \tfile.txt\x00"),
		"header-tab":       []byte("100644\t" + strings.Repeat("1", 40) + " 0\tfile.txt\x00"),
		"unmerged-stage":   trackedInventoryItem("100644", strings.Repeat("1", 40), "1", "file.txt"),
		"malformed-oid":    trackedInventoryItem("100644", "not-an-oid", "0", "file.txt"),
		"uppercase-oid":    trackedInventoryItem("100644", strings.Repeat("A", 40), "0", "file.txt"),
		"unsupported-mode": trackedInventoryItem("100600", strings.Repeat("1", 40), "0", "file.txt"),
		"invalid-path":     trackedInventoryItem("100644", strings.Repeat("1", 40), "0", "../file.txt"),
		"empty-record":     append(append([]byte{}, valid...), 0),
	}
	for name, raw := range tests {
		t.Run(name, func(t *testing.T) {
			if _, _, err := parseTracked(context.Background(), raw, 40); err == nil {
				t.Fatal("invalid tracked inventory was accepted")
			}
		})
	}
}

func TestParseTrackedRequiresObjectFormatSpecificOIDLength(t *testing.T) {
	sha1 := trackedInventoryItem("100644", strings.Repeat("1", 40), "0", "file.txt")
	sha256 := trackedInventoryItem("100644", strings.Repeat("2", 64), "0", "file.txt")
	for _, test := range []struct {
		name   string
		raw    []byte
		length int
	}{
		{"sha1-as-sha256", sha1, 64},
		{"sha256-as-sha1", sha256, 40},
	} {
		t.Run(test.name, func(t *testing.T) {
			if _, _, err := parseTracked(context.Background(), test.raw, test.length); err == nil {
				t.Fatal("object-format-mismatched tracked OID was accepted")
			}
		})
	}
}

func TestAppendUntrackedRejectsDuplicateOverlapAndMalformedRecords(t *testing.T) {
	seed := []inventoryRecord{{path: "tracked.txt", tracking: "tracked"}}
	tests := map[string][]byte{
		"duplicate":    []byte("new.txt\x00new.txt\x00"),
		"overlap":      []byte("tracked.txt\x00"),
		"invalid-path": []byte("../new.txt\x00"),
		"missing-nul":  []byte("new.txt"),
		"empty-record": []byte("\x00"),
	}
	for name, raw := range tests {
		t.Run(name, func(t *testing.T) {
			seen := map[string]struct{}{"tracked.txt": {}}
			if _, _, err := appendUntracked(context.Background(), raw, seed, seen); err == nil {
				t.Fatal("invalid untracked inventory was accepted")
			}
		})
	}
}

func TestCountIgnoredRejectsDuplicateMalformedAndBoundOverflow(t *testing.T) {
	tests := map[string][]byte{
		"duplicate":    []byte("one\x00one\x00"),
		"invalid-path": []byte("../one\x00"),
		"missing-nul":  []byte("one"),
		"empty-record": []byte("\x00"),
	}
	for name, raw := range tests {
		t.Run(name, func(t *testing.T) {
			if _, err := countIgnoredBounded(context.Background(), raw, nil, 10); err == nil {
				t.Fatal("invalid ignored inventory was accepted")
			}
		})
	}
	if count, err := countIgnoredBounded(context.Background(), []byte("one\x00two\x00"), nil, 2); err != nil || count != 2 {
		t.Fatalf("at-bound ignored count = %d, %v", count, err)
	}
	if count, err := countIgnoredBounded(context.Background(), []byte("one\x00two\x00three\x00"), nil, 2); err == nil || count != 3 {
		t.Fatalf("overflow ignored count = %d, %v", count, err)
	}
	if _, err := countIgnoredBounded(context.Background(), []byte("tracked.txt\x00"),
		map[string]struct{}{"tracked.txt": {}}, 2); err == nil {
		t.Fatal("ignored/source overlap was accepted")
	}
}

func trackedInventoryItem(mode, oid, stage, path string) []byte {
	return []byte(mode + " " + oid + " " + stage + "\t" + path + "\x00")
}
