package test

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"example/taskd/internal/domain"
	"example/taskd/internal/httpapi"
	"example/taskd/internal/service"
	"example/taskd/internal/store"
)

// newServer spins up a real httptest server over the full wired stack.
func newServer(t *testing.T) *httptest.Server {
	t.Helper()
	srv := httptest.NewServer(httpapi.New(service.New(store.NewMemory())))
	t.Cleanup(srv.Close)
	return srv
}

// do issues a request and returns the status code and raw body.
func do(t *testing.T, method, url, body string) (int, string) {
	t.Helper()
	req, err := http.NewRequest(method, url, strings.NewReader(body))
	if err != nil {
		t.Fatalf("build request: %v", err)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("%s %s: %v", method, url, err)
	}
	defer func() {
		if err := resp.Body.Close(); err != nil {
			t.Errorf("close response body: %v", err)
		}
	}()
	b, _ := io.ReadAll(resp.Body)
	return resp.StatusCode, string(b)
}

// createTask POSTs a title and returns the decoded created task.
func createTask(t *testing.T, base, title string) domain.Task {
	t.Helper()
	payload, _ := json.Marshal(map[string]string{"title": title})
	status, body := do(t, http.MethodPost, base+"/tasks", string(payload))
	if status != http.StatusCreated {
		t.Fatalf("POST /tasks status = %d (%s), want 201", status, body)
	}
	var task domain.Task
	if err := json.Unmarshal([]byte(body), &task); err != nil {
		t.Fatalf("decode created task: %v (%s)", err, body)
	}
	return task
}

func TestPostCreates201(t *testing.T) {
	srv := newServer(t)
	task := createTask(t, srv.URL, "write tests")
	if task.ID == "" || task.Title != "write tests" || task.Done {
		t.Fatalf("unexpected created task: %#v", task)
	}
}

func TestGetListAndItem200(t *testing.T) {
	srv := newServer(t)
	task := createTask(t, srv.URL, "alpha")

	status, body := do(t, http.MethodGet, srv.URL+"/tasks", "")
	if status != http.StatusOK {
		t.Fatalf("GET /tasks status = %d, want 200", status)
	}
	var list []domain.Task
	if err := json.Unmarshal([]byte(body), &list); err != nil || len(list) != 1 {
		t.Fatalf("GET /tasks body = %s (err %v)", body, err)
	}

	status, body = do(t, http.MethodGet, srv.URL+"/tasks/"+task.ID, "")
	if status != http.StatusOK {
		t.Fatalf("GET /tasks/{id} status = %d, want 200 (%s)", status, body)
	}
}

func TestGetMissing404(t *testing.T) {
	srv := newServer(t)
	if status, _ := do(t, http.MethodGet, srv.URL+"/tasks/nope", ""); status != http.StatusNotFound {
		t.Fatalf("GET missing status = %d, want 404", status)
	}
}

func TestPatchCompletes200AndPersists(t *testing.T) {
	srv := newServer(t)
	task := createTask(t, srv.URL, "finish me")

	status, body := do(t, http.MethodPatch, srv.URL+"/tasks/"+task.ID, "")
	if status != http.StatusOK {
		t.Fatalf("PATCH status = %d, want 200", status)
	}
	var got domain.Task
	_ = json.Unmarshal([]byte(body), &got)
	if !got.Done {
		t.Fatalf("PATCH response not Done: %#v", got)
	}

	_, body = do(t, http.MethodGet, srv.URL+"/tasks/"+task.ID, "")
	_ = json.Unmarshal([]byte(body), &got)
	if !got.Done {
		t.Fatal("completion did not persist")
	}
}

func TestPatchMissing404(t *testing.T) {
	srv := newServer(t)
	if status, _ := do(t, http.MethodPatch, srv.URL+"/tasks/ghost", ""); status != http.StatusNotFound {
		t.Fatalf("PATCH missing status = %d, want 404", status)
	}
}

func TestDelete204Then404(t *testing.T) {
	srv := newServer(t)
	task := createTask(t, srv.URL, "remove me")

	if status, _ := do(t, http.MethodDelete, srv.URL+"/tasks/"+task.ID, ""); status != http.StatusNoContent {
		t.Fatalf("DELETE status = %d, want 204", status)
	}
	if status, _ := do(t, http.MethodDelete, srv.URL+"/tasks/"+task.ID, ""); status != http.StatusNotFound {
		t.Fatalf("DELETE again status = %d, want 404", status)
	}
}

func TestMalformedBody400(t *testing.T) {
	srv := newServer(t)
	if status, _ := do(t, http.MethodPost, srv.URL+"/tasks", "{not json"); status != http.StatusBadRequest {
		t.Fatalf("malformed body status = %d, want 400", status)
	}
	if status, _ := do(t, http.MethodPost, srv.URL+"/tasks", `{"title":""}`); status != http.StatusBadRequest {
		t.Fatalf("empty title status = %d, want 400", status)
	}
}

// TestBadTargetKeepsServerUp fires hostile/odd targets, then proves the server
// still serves a normal request — a bad request must never crash the process.
func TestBadTargetKeepsServerUp(t *testing.T) {
	srv := newServer(t)
	bad := []struct {
		method, path, body string
	}{
		{http.MethodGet, "/tasks/a/b/c", ""},
		{http.MethodGet, "/tasks/", ""},
		{http.MethodPost, "/tasks", strings.Repeat("x", 4096)},
		{http.MethodPut, "/tasks/1", ""},
	}
	for _, c := range bad {
		// We only require the server to RESPOND (not crash); status is not asserted.
		do(t, c.method, srv.URL+c.path, c.body)
	}
	// Server must still be healthy after the barrage.
	task := createTask(t, srv.URL, "still alive")
	if task.ID == "" {
		t.Fatal("server failed to serve after bad-target barrage")
	}
}

// TestOversizedBodyRejected confirms the 1 MiB cap rejects huge payloads
// without crashing — exercises http.MaxBytesReader in createTask handler.
func TestOversizedBodyRejected(t *testing.T) {
	srv := newServer(t)
	huge := bytes.Repeat([]byte("a"), (1<<20)+1024)
	status, _ := do(t, http.MethodPost, srv.URL+"/tasks", `{"title":"`+string(huge)+`"}`)
	if status == http.StatusCreated {
		t.Fatalf("oversized body unexpectedly accepted (status %d)", status)
	}
}
