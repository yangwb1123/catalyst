package graphscheduledcontract

import (
	"os"
	"os/exec"
	"strconv"
	"strings"
	"testing"
)

// specValue runs harness/spec_check.py to read one key from the
// scheduled-successor-protocol.md authority.
func specValue(t *testing.T, table, key string) string {
	t.Helper()
	out, err := execSpecCheck(table, key)
	if err != nil {
		t.Fatalf("spec_check %s/%s: %v", table, key, err)
	}
	return strings.TrimSpace(out)
}

func execSpecCheck(table, key string) (string, error) {
	// 本包位于 forge-core/internal/graphscheduledcontract,spec 脚本在仓库根。
	cmd := exec.Command("python3",
		"../../../harness/spec_check.py", "--table", table, "--key", key)
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return string(out), nil
}

func specUint(t *testing.T, table, key string) uint64 {
	t.Helper()
	value, err := strconv.ParseUint(specValue(t, table, key), 10, 64)
	if err != nil {
		t.Fatalf("spec %s/%s is not an integer: %q", table, key, specValue(t, table, key))
	}
	return value
}

func TestSpecVersionsMatchGoConstants(t *testing.T) {
	if got := uint64(CandidateVersion); got != specUint(t, "versions", "candidate.v") {
		t.Fatalf("CandidateVersion = %d, spec says %d", got, specUint(t, "versions", "candidate.v"))
	}
	if got := uint64(RequestVersion); got != specUint(t, "versions", "request.v") {
		t.Fatalf("RequestVersion = %d, spec says %d", got, specUint(t, "versions", "request.v"))
	}
	if got := uint64(NodeExecutionProtocolVersion); got != specUint(t, "versions", "node_execution_protocol_version") {
		t.Fatalf("NodeExecutionProtocolVersion = %d, spec says %d", got, specUint(t, "versions", "node_execution_protocol_version"))
	}
}

func specDomain(t *testing.T, key string) string {
	t.Helper()
	// spec md 用 \x00 字面表示域分隔符;unescape 成真实字节。
	quoted, err := strconv.Unquote(`"` + specValue(t, "digests", key) + `"`)
	if err != nil {
		t.Fatalf("spec digest %s unescape: %v", key, err)
	}
	return quoted
}

func TestSpecDigestDomainsMatchGoConstants(t *testing.T) {
	if got := contractDigestDomain; got != specDomain(t, "contract_digest_domain") {
		t.Fatalf("contractDigestDomain = %q, spec says %q", got, specDomain(t, "contract_digest_domain"))
	}
	if got := requestDigestDomain; got != specDomain(t, "request_digest_domain") {
		t.Fatalf("requestDigestDomain = %q, spec says %q", got, specDomain(t, "request_digest_domain"))
	}
}

func TestSpecBoundsMatchGoConstants(t *testing.T) {
	if got := uint64(MaxCandidateBytes); got != specUint(t, "bounds", "contract_bytes.max") {
		t.Fatalf("MaxCandidateBytes = %d, spec says %d", got, specUint(t, "bounds", "contract_bytes.max"))
	}
	if got := uint64(MaxPredecessorOutputBytes); got != specUint(t, "bounds", "predecessor_output_bytes.max") {
		t.Fatalf("MaxPredecessorOutputBytes = %d, spec says %d", got, specUint(t, "bounds", "predecessor_output_bytes.max"))
	}
	if got := uint64(MaxUserPromptBytes); got != specUint(t, "bounds", "user_prompt_bytes.max") {
		t.Fatalf("MaxUserPromptBytes = %d, spec says %d", got, specUint(t, "bounds", "user_prompt_bytes.max"))
	}
	if got := uint64(maxSuccessorOrdinal); got != specUint(t, "bounds", "successor.execution_ordinal.max") {
		t.Fatalf("maxSuccessorOrdinal = %d, spec says %d", got, specUint(t, "bounds", "successor.execution_ordinal.max"))
	}
	if got := uint64(maxPredecessorCount); got != specUint(t, "bounds", "predecessor_receipt_count.max") {
		t.Fatalf("maxPredecessorCount = %d, spec says %d", got, specUint(t, "bounds", "predecessor_receipt_count.max"))
	}
	if got := uint64(maxTopologyWaveIndex); got != specUint(t, "bounds", "successor.execution_ordinal.max") {
		t.Fatalf("maxTopologyWaveIndex = %d, spec says %d", got, specUint(t, "bounds", "successor.execution_ordinal.max"))
	}
}

func TestSpecSuccessorInvariantsMatchGoProtocol(t *testing.T) {
	expected := map[string]string{
		"successor.attempt":                      "1",
		"receipt.attempt":                        "1",
		"successor.retry_authorized":             "false",
		"receipt.retry_authorized":               "false",
		"receipt.successor_advance_authorized":   "false",
		"successor.predecessor_content_included": "optional;`true` iff the user Prompt contains exact `predecessor_output`(ADR-0033)",
		"successor.predecessor_content_source":   "仅当 included=true:canonical ordered direct-receipt closure 的第一份 receipt 所绑定的 durable terminalized result artifact",
		"wave_sibling.receipts":                  "0(空直接前驱集,ADR-0035)",
	}
	for key, want := range expected {
		if got := specValue(t, "invariants", key); got != want {
			t.Fatalf("invariant %s = %q, want %q", key, got, want)
		}
	}
}

func TestSpecIdentityPrefixesMatchGo(t *testing.T) {
	if got := contractIDPrefix; got != specValue(t, "identities", "contract_id_prefix") {
		t.Fatalf("contract id prefix = %q, spec says %q", got, specValue(t, "identities", "contract_id_prefix"))
	}
	if got := requestIDPrefix; got != specValue(t, "identities", "request_id_prefix") {
		t.Fatalf("request id prefix = %q, spec says %q", got, specValue(t, "identities", "request_id_prefix"))
	}
}

func TestSpecFileExists(t *testing.T) {
	if _, err := os.Stat("../../../docs/contracts/scheduled-successor-protocol.md"); err != nil {
		t.Fatalf("authority spec missing: %v", err)
	}
}
