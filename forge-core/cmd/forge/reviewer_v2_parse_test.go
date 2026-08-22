package main

import (
	"strings"
	"testing"
)

func TestReviewerV2VerdictRequiresOneExactBindingAndVerdictTail(t *testing.T) {
	binding := strings.Repeat("a", 64)
	valid := "findings\n" + reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n"
	if verdict, ok := parseReviewerV2Verdict(valid, binding); !ok || verdict != VerdictApprove {
		t.Fatalf("valid reviewer_v2 payload = (%q, %v)", verdict, ok)
	}
	for _, payload := range []string{
		"VERDICT: APPROVE\n",
		reviewerBindingPrefix + strings.Repeat("0", 64) + "\nVERDICT: APPROVE\n",
		reviewerBindingPrefix + strings.Repeat("0", 64) + "\n" + valid,
		reviewerBindingToken + "not-a-binding\n" + valid,
		"> " + reviewerBindingPrefix + strings.Repeat("0", 64) + "\n" + valid,
		"  " + reviewerBindingPrefix + strings.Repeat("0", 64) + "\n" + valid,
		reviewerBindingPrefix + binding + "\nVERDICT: REQUEST_CHANGES\nVERDICT: APPROVE\n",
		"> VERDICT: REQUEST_CHANGES\n" + reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n",
		reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\ntrailing",
		" " + reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n",
		reviewerBindingPrefix + binding + "\r\nVERDICT: APPROVE\r\n",
		string([]byte{0xff}) + "\n" + reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n",
	} {
		if verdict, ok := parseReviewerV2Verdict(payload, binding); ok || verdict != "" {
			t.Errorf("invalid reviewer_v2 payload %q = (%q, %v)", payload, verdict, ok)
		}
	}
}
