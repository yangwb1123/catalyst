package graphscheduledrelease

import (
	"strings"
	"testing"
)

func TestReadyAuthorizationRejectsResignedPolicyAndFactDrift(t *testing.T) {
	control, _ := validReadyInitialFixture(t)
	base, err := BuildReadyAuthorization(control)
	if err != nil {
		t.Fatalf("BuildReadyAuthorization: %v", err)
	}
	cases := []struct {
		name   string
		mutate func(*ReadyAuthorization)
	}{
		{"max future releases", func(v *ReadyAuthorization) { v.MaximumFutureNodeReleases = 2 }},
		{"atomic requirement", func(v *ReadyAuthorization) { v.ReleaseRequirements.AtomicTransition = "other" }},
		{"successor requirement", func(v *ReadyAuthorization) { v.ReleaseRequirements.Successor = "other" }},
		{"provider sent", func(v *ReadyAuthorization) { v.ProviderRequestSent = true }},
		{"successor effect", func(v *ReadyAuthorization) { v.SuccessorAdvanceAuthorized = true }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := base
			test.mutate(&value)
			resignReadyAuthorizationTest(t, &value)
			if _, err := MarshalReadyAuthorization(value); err == nil {
				t.Fatal("resigned policy or current-fact drift accepted")
			}
		})
	}
}

func TestReadyAuthorizationUsesDistinctDomainAndPrefix(t *testing.T) {
	legacyControl, _ := validReleaseFixture(t)
	legacy, err := BuildAuthorization(legacyControl)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	readyControl, _ := validReadyInitialFixture(t)
	ready, err := BuildReadyAuthorization(readyControl)
	if err != nil {
		t.Fatalf("BuildReadyAuthorization: %v", err)
	}
	if !strings.HasPrefix(ready.AuthorizationID, readyAuthorizationIDPrefix) ||
		strings.HasPrefix(ready.AuthorizationID, authorizationIDPrefix) ||
		ready.AuthorizationSHA256 == legacy.AuthorizationSHA256 {
		t.Fatal("ready v2 authorization was not domain and prefix separated from v1")
	}
}
