package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestPersistedObservationV2CommandConsumesSharedFixture(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2CommandFixture(t)
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"persisted-observation-v2", "--input", "-"}, bytes.NewReader(fixture), &stdout, &stderr); code != 0 || stderr.Len() != 0 {
		t.Fatalf("command = %d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	var result PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal(stdout.Bytes(), &result); err != nil {
		t.Fatalf("decode result: %v", err)
	}
	if result.SchemaVersion != PersistedInventoryPlacementV2SchemaVersion || result.SelectedDeviceID != nil || result.SelectedInstanceID != nil ||
		result.Authority != (PersistedInventoryPlacementBatchAuthority{}) || len(result.Decisions) != 2 {
		t.Fatalf("unexpected v2 result: %#v", result)
	}
}

func TestPersistedObservationV2CommandFailsClosed(t *testing.T) {
	fixture := readPersistedInventoryPlacementV2CommandFixture(t)
	inputs := [][]byte{
		bytes.Replace(fixture, []byte(`"authority": {`), []byte(`"unexpected":true,"authority": {`), 1),
		bytes.Replace(fixture, []byte(`"schema_version"`), []byte(`"schema_version":"duplicate","schema_version"`), 1),
		bytes.Replace(fixture, []byte(`"placement_selected": false`), []byte(`"placement_selected": true`), 1),
		bytes.Repeat([]byte{' '}, MaxRequestBytes+1),
	}
	for _, input := range inputs {
		var stdout, stderr bytes.Buffer
		if code := Command([]string{"persisted-observation-v2", "--input", "-"}, bytes.NewReader(input), &stdout, &stderr); code != 1 || stdout.Len() != 0 {
			t.Errorf("command = %d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
		}
	}
}

func readPersistedInventoryPlacementV2CommandFixture(t *testing.T) []byte {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-device-inventory-placement-evaluation-v2.json")
	fixture, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return fixture
}
