package gitworktreesource

import (
	"context"
	"strconv"
	"strings"
	"testing"
)

func TestTrackedInventoryRejectsEntryBeyondLimitBeforeParsingIt(t *testing.T) {
	var raw strings.Builder
	for index := 0; index < maxSourceEntries; index++ {
		raw.WriteString("100644 000000 0\tf")
		raw.WriteString(strconv.Itoa(index))
		raw.WriteByte(0)
	}
	raw.WriteString("malformed-entry-that-must-not-be-parsed")
	if _, _, err := parseTrackedInventory(context.Background(), []byte(raw.String())); err == nil || !strings.Contains(err.Error(), "exceeds 65536 entries") {
		t.Fatalf("tracked overflow error = %v", err)
	}
}

func TestCombinedInventoryRejectsFirstEntryBeyondLimit(t *testing.T) {
	records := make([]inventoryRecord, maxSourceEntries)
	seen := make(map[string]struct{}, maxSourceEntries)
	for index := range records {
		path := "f" + strconv.Itoa(index)
		records[index] = inventoryRecord{path: path, tracking: "tracked"}
		seen[path] = struct{}{}
	}
	result, err := appendUntrackedInventory(
		context.Background(), []byte("new-path\x00second-path\x00"), records, seen)
	if err == nil || !strings.Contains(err.Error(), "exceeds 65536 entries") ||
		len(result) != maxSourceEntries {
		t.Fatalf("combined overflow result=%d error=%v", len(result), err)
	}
}

func TestNULIteratorStopsAtVisitorError(t *testing.T) {
	visits := 0
	want := context.Canceled
	err := forEachNUL([]byte("one\x00two\x00three\x00"), func([]byte) error {
		visits++
		return want
	})
	if err != want || visits != 1 {
		t.Fatalf("iterator error=%v visits=%d", err, visits)
	}
}
