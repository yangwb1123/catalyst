package projectsnapshot

import (
	"strings"
	"testing"
)

func TestManifestFinalSelfSealCannotCrossCanonicalBound(t *testing.T) {
	value := SourceManifest{
		APIVersion: manifestVersion, Canonicalization: canonicalization,
		Entries: []Entry{}, Excluded: []Exclusion{}, GitObserver: GitObserver{},
		PathPolicyID: pathPolicyID, ProfileID: profileID,
	}
	base, err := canonicalJSON(value, maxManifestBytes)
	if err != nil {
		t.Fatal(err)
	}
	value.GitObserver.Version = strings.Repeat("x", maxManifestBytes-len(base))
	emptySeal, err := canonicalJSON(value, maxManifestBytes)
	if err != nil || len(emptySeal) != maxManifestBytes {
		t.Fatalf("empty-seal boundary = %d bytes, %v", len(emptySeal), err)
	}
	if err := sealManifest(&value); err == nil {
		t.Fatal("final 64-hex self seal crossed manifest bound")
	}
}
