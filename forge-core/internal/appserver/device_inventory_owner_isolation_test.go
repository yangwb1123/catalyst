package appserver

import (
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestDeviceInventoryReadCandidateRejectsEveryForeignOwnerTupleField(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	foreignOwner := deviceplacement.Owner{Issuer: identity.issuer, Subject: wantOwner.Subject, TenantID: wantOwner.TenantID}
	cases := []struct {
		name   string
		mutate func(*deviceplacement.Owner)
	}{
		{name: "issuer", mutate: func(owner *deviceplacement.Owner) { owner.Issuer = "https://foreign.example" }},
		{name: "subject", mutate: func(owner *deviceplacement.Owner) { owner.Subject = "account-foreign" }},
		{name: "tenant", mutate: func(owner *deviceplacement.Owner) { owner.TenantID = "tenant-foreign" }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			declared := foreignOwner
			test.mutate(&declared)
			source := &fixtureDeviceInventoryReadSource{
				value: fixtureDeviceInventoryReadValue(declared),
			}
			handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
				Enabled: true,
				Source:  source,
			}))
			response := requestConversationAPI(t, handler, identity, http.MethodGet,
				deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
			if response.Code != http.StatusBadGateway ||
				!strings.Contains(response.Body.String(), `"code":"device_inventory_invalid"`) {
				t.Fatalf("foreign %s response status=%d body=%q", test.name, response.Code, response.Body.String())
			}
			if source.calls != 1 || source.owner != wantOwner {
				t.Fatalf("foreign %s source calls=%d owner=%#v want %#v", test.name, source.calls, source.owner, wantOwner)
			}
		})
	}
}
