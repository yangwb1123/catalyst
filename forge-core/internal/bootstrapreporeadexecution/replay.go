package bootstrapreporeadexecution

import (
	"encoding/hex"
	"fmt"

	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
)

type replayStatus struct {
	active   bool
	conflict bool
	digests  bool
	found    bool
	output   []byte
	state    string
}

func tryReplay(ledger *bootstraprepoexecutionauthority.Ledger,
	identity identityBytes) (replayStatus, error) {
	if identity.invocationErr != nil {
		if len(identity.policy) == 64 {
			return replayStatus{}, coded(CodeInvocationRejected,
				fmt.Errorf("digest-only replay requires both identity leaves"))
		}
		return replayStatus{}, nil
	}
	digests, err := replayDigestPair(identity)
	if err != nil {
		return replayStatus{}, coded(CodeInvocationRejected, err)
	}
	if ledger == nil {
		return replayStatus{digests: digests}, nil
	}
	delivery, state, found, conflict, err := ledger.Replay(
		identity.policy, identity.invocation)
	if err != nil {
		return replayStatus{}, coded(CodeInvocationRejected, err)
	}
	status := replayStatus{conflict: conflict, digests: digests, found: found, state: state}
	if conflict || !found {
		return status, nil
	}
	if delivery == nil {
		status.active = state == "reserved_no_repo_io" || state == "effect_intent"
		return status, nil
	}
	encoded, err := bootstraprepoexecutionauthority.CanonicalJSON(delivery)
	if err != nil {
		return replayStatus{}, coded(CodeLedgerRejected, err)
	}
	status.output = encoded
	return status, nil
}

func replayDigestPair(identity identityBytes) (bool, error) {
	policySized := len(identity.policy) == 64
	invocationSized := len(identity.invocation) == 64
	if !policySized && !invocationSized {
		return false, nil
	}
	if !lowerDigestBytes(identity.policy) || !lowerDigestBytes(identity.invocation) {
		return false, fmt.Errorf("replay identity leaves must both contain lowercase SHA-256")
	}
	return true, nil
}

func lowerDigestBytes(value []byte) bool {
	if len(value) != 64 {
		return false
	}
	decoded, err := hex.DecodeString(string(value))
	return err == nil && len(decoded) == 32 && hex.EncodeToString(decoded) == string(value)
}
