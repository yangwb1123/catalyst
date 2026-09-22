package appserver

import (
	"encoding/json"
	"net/http"
	"testing"
)

func decodeSnaplinkResponse(t *testing.T, response *http.Response, destination any) bool {
	t.Helper()
	defer response.Body.Close()
	return json.NewDecoder(response.Body).Decode(destination) == nil
}
