//go:build aix || solaris

package authenticatedadrlifecycleauthority

import (
	"strings"
	"testing"
)

func TestAuthorityFailsClosedOnRecordLockOnlyUnix(t *testing.T) {
	config := Config{RepositoryRoot: "/repository", AuthorityRoot: "/authority",
		StateDir: "state", SignatureProfilePath: "profile.json",
		ApprovalTrustRootPath: "approval-root.json", LifecycleTrustRootPath: "lifecycle-root.json",
		StateSignerSeedPath: "lifecycle-state.seed"}
	trust := ExternalTrust{PinnedApprovalTrustRootSHA256: strings.Repeat("a", 64),
		PinnedApprovalTrustEpoch: 1, PinnedLifecycleTrustRootSHA256: strings.Repeat("b", 64),
		PinnedLifecycleTrustEpoch: 1, ObservedAtUnixMS: 0}
	_, err := TransitionAndStore(config, EncodedTransitionInput{}, nil, trust)
	if ErrorCode(err) != codeUnsupported {
		t.Fatalf("code=%q want=%q", ErrorCode(err), codeUnsupported)
	}
}
