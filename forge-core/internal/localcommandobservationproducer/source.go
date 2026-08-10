package localcommandobservationproducer

import (
	"context"

	"forgeos/forge-core/internal/gitworktreesource"
)

const maxIndividualFileBytes = int64(1 << 30)

// sourceSnapshot preserves the ADR-0051 package-private shape while the
// neutral owner performs capture, canonical validation and digesting.
func sourceSnapshot(ctx context.Context, root string, childEnvironment []string) (SourceManifest, string, error) {
	snapshot, err := gitworktreesource.Capture(ctx, root, childEnvironment)
	if err != nil {
		return SourceManifest{}, "", err
	}
	return snapshot.Manifest, snapshot.SHA256, nil
}

func lowerHex(value string) bool {
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return value != ""
}
