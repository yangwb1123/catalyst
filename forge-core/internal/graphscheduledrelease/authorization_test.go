package graphscheduledrelease

import "testing"

func TestAuthorizationRejectsResignedPolicyAndFactDrift(t *testing.T) {
	control, _ := validReleaseFixture(t)
	authorization, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	cases := []struct {
		name   string
		mutate func(*Authorization)
	}{
		{"health check", func(v *Authorization) { v.ReleaseRequirements.ProviderHealthCheck = "allowed" }},
		{"atomic transition", func(v *Authorization) { v.ReleaseRequirements.AtomicTransition = "separate" }},
		{"admission decision", func(v *Authorization) { v.LifecycleContractAdmissionAuthorized = false }},
		{"release decision", func(v *Authorization) { v.DispatchAuthorityReleaseAuthorized = false }},
		{"candidate fact", func(v *Authorization) { v.ScheduledContractCandidatePresent = false }},
		{"admitted fact", func(v *Authorization) { v.LifecycleContractAdmitted = true }},
		{"lane fact", func(v *Authorization) { v.ProjectLaneClaimed = true }},
		{"terminal fact", func(v *Authorization) { v.TerminalReceiptRecorded = true }},
		{"successor fact", func(v *Authorization) { v.SuccessorAdvanceAuthorized = true }},
		{"budget", func(v *Authorization) { v.Budgets.MaxTurns = 2 }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			mutated := authorization
			test.mutate(&mutated)
			resignAuthorizationTest(t, &mutated)
			if _, err := MarshalAuthorization(mutated); err == nil {
				t.Fatal("resigned authorization drift accepted")
			}
		})
	}
}

func TestScheduledAuthorizationUsesDistinctDigestDomainAndPrefix(t *testing.T) {
	control, _ := validReleaseFixture(t)
	value, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	legacy := mustDomainDigestTest(t, "forge.group-agent-node-dispatch-authorization.v1\x00",
		authorizationPayloadFrom(value))
	if legacy == value.AuthorizationSHA256 ||
		value.AuthorizationID[:len(authorizationIDPrefix)] != authorizationIDPrefix {
		t.Fatal("scheduled authorization reused a legacy identity domain")
	}
}

func resignAuthorizationTest(t *testing.T, value *Authorization) {
	t.Helper()
	value.AuthorizationSHA256 = mustDomainDigestTest(t, authorizationDigestDomain,
		authorizationPayloadFrom(*value))
	value.AuthorizationID = authorizationIDPrefix + value.AuthorizationSHA256
}
