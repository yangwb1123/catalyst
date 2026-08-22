package outputbindingstore

import (
	"bytes"
	"encoding/json"
	"fmt"
	"path/filepath"

	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/statefs"
)

type receiptAnchor struct {
	Format        string `json:"_format"`
	Count         int    `json:"count"`
	JournalSHA256 string `json:"journal_sha256"`
}

// ReceiptAnchorPath identifies the detached monotonic witness for the exact
// receipt journal. It is intentionally separate from the preflight-claim head.
func ReceiptAnchorPath(root string) string {
	return filepath.Join(root, ".forge", receiptAnchorName)
}

func (store *Store) loadAnchoredLedger(limits ledgerLimits) (ledgerSnapshot, error) {
	snapshot, err := loadLedger(Path(store.root), limits)
	if err != nil {
		return ledgerSnapshot{}, err
	}
	anchor, present, err := readReceiptAnchor(store.root)
	if err != nil {
		return ledgerSnapshot{}, err
	}
	if !present {
		if len(snapshot.receipts) > 0 {
			return ledgerSnapshot{}, fmt.Errorf("output binding store: receipt journal anchor is missing")
		}
		return snapshot, nil
	}
	if anchor.Count > len(snapshot.receipts) {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: receipt journal rolled back below anchored head")
	}
	prefix := claimPrefix(snapshot.data, anchor.Count)
	if outputbinding.SHA256(prefix) != anchor.JournalSHA256 {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: receipt journal differs from anchored prefix")
	}
	if anchor.Count < len(snapshot.receipts) {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: receipt journal anchor lags the journal")
	}
	return snapshot, nil
}

func writeReceiptAnchor(root string, journal []byte, count int) error {
	anchor := receiptAnchor{
		Format: receiptAnchorFormat, Count: count,
		JournalSHA256: outputbinding.SHA256(journal),
	}
	data, err := json.Marshal(anchor)
	if err != nil {
		return fmt.Errorf("output binding store: encode receipt journal anchor: %w", err)
	}
	if err := statefs.AtomicWrite(ReceiptAnchorPath(root), data, 0o600); err != nil {
		return fmt.Errorf("output binding store: commit receipt journal anchor: %w", err)
	}
	return nil
}

func readReceiptAnchor(root string) (receiptAnchor, bool, error) {
	data, present, err := statefs.ReadRegularUnmodified(ReceiptAnchorPath(root), 1<<10)
	if err != nil || !present {
		return receiptAnchor{}, present, err
	}
	var anchor receiptAnchor
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&anchor); err != nil {
		return receiptAnchor{}, true, fmt.Errorf("output binding store: decode receipt journal anchor: %w", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err == nil {
		return receiptAnchor{}, true, fmt.Errorf("output binding store: receipt journal anchor has trailing JSON")
	}
	canonical, err := json.Marshal(anchor)
	if err != nil || !bytes.Equal(canonical, data) || anchor.Format != receiptAnchorFormat ||
		anchor.Count < 1 || anchor.Count > defaultMaxReceipts ||
		!validClaimDigest(anchor.JournalSHA256) {
		return receiptAnchor{}, true, fmt.Errorf("output binding store: invalid receipt journal anchor")
	}
	return anchor, true, nil
}
