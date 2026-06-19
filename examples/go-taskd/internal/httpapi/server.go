// Package httpapi is the HTTP presentation layer (net/http, stdlib only).
//
// Clean Architecture: this layer holds NO business rules. It parses requests,
// dispatches to the injected *service.Service, and serializes responses. It
// depends inward on the service (and, transitively, the domain) and on nothing
// else. Every handler is wrapped so a malformed request can never panic the
// process — the DoS lesson from the url-shortener fix.
//
// Routes:
//
//	POST   /tasks        {"title":"..."} -> 201 {id,title,done} | 400
//	GET    /tasks                        -> 200 [task,...]
//	GET    /tasks/{id}                   -> 200 task | 404
//	PATCH  /tasks/{id}   (mark done)     -> 200 task | 404
//	DELETE /tasks/{id}                   -> 204 | 404
package httpapi

import (
	"encoding/json"
	"errors"
	"net/http"
	"strings"

	"example/taskd/internal/domain"
	"example/taskd/internal/service"
)

// svc is the minimal port the handlers need from the application layer. The
// concrete *service.Service satisfies it; defining it here keeps the dependency
// arrow pointing inward and makes the handlers trivially testable.
type svc interface {
	Create(title string) (domain.Task, error)
	List() []domain.Task
	Get(id string) (domain.Task, error)
	Complete(id string) (domain.Task, error)
	Delete(id string) error
}

// maxBody caps request bodies (1 MiB) so an oversized payload cannot exhaust
// memory — a malformed/hostile body must never take the server down.
const maxBody = 1 << 20

// New builds an http.Handler bound to the given service. The returned handler
// recovers from any panic in a handler and replies 500 rather than crashing the
// process, guaranteeing a bad request can never bring the server down.
func New(s *service.Service) http.Handler {
	mux := http.NewServeMux()
	register(mux, s)
	return recoverMiddleware(mux)
}

// register wires routes onto mux. Split out so New stays tiny and the handler
// port (svc) — not the concrete type — is what the closures actually use.
func register(mux *http.ServeMux, s svc) {
	mux.HandleFunc("/tasks", func(w http.ResponseWriter, r *http.Request) {
		collection(w, r, s)
	})
	mux.HandleFunc("/tasks/", func(w http.ResponseWriter, r *http.Request) {
		item(w, r, s)
	})
}

// recoverMiddleware turns any handler panic into a 500 instead of a crash.
func recoverMiddleware(h http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		defer func() {
			if rec := recover(); rec != nil {
				writeError(w, http.StatusInternalServerError, "internal error")
			}
		}()
		h.ServeHTTP(w, r)
	})
}

// collection routes the /tasks endpoint (no id): POST creates, GET lists.
func collection(w http.ResponseWriter, r *http.Request, s svc) {
	switch r.Method {
	case http.MethodPost:
		createTask(w, r, s)
	case http.MethodGet:
		writeJSON(w, http.StatusOK, s.List())
	default:
		writeError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}

// item routes the /tasks/{id} endpoint: GET reads, PATCH completes, DELETE
// removes. The id is extracted defensively; an empty/bad id yields 400.
func item(w http.ResponseWriter, r *http.Request, s svc) {
	id, ok := parseID(r.URL.Path)
	if !ok {
		writeError(w, http.StatusBadRequest, "bad task id")
		return
	}
	switch r.Method {
	case http.MethodGet:
		t, err := s.Get(id)
		respondTask(w, t, err)
	case http.MethodPatch:
		t, err := s.Complete(id)
		respondTask(w, t, err)
	case http.MethodDelete:
		deleteTask(w, s, id)
	default:
		writeError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}

// parseID extracts a single non-empty path segment after "/tasks/". A trailing
// slash, nested path, or empty id is rejected (ok=false) so a malformed target
// can never reach the service as a surprising key.
func parseID(path string) (string, bool) {
	rest := strings.TrimPrefix(path, "/tasks/")
	if rest == "" || strings.Contains(rest, "/") {
		return "", false
	}
	return rest, true
}

// createTask decodes {"title":...} and creates a task. A malformed JSON body or
// a validation error (empty title) both yield 400 — never a crash.
func createTask(w http.ResponseWriter, r *http.Request, s svc) {
	var in struct {
		Title string `json:"title"`
	}
	dec := json.NewDecoder(http.MaxBytesReader(w, r.Body, maxBody))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&in); err != nil {
		writeError(w, http.StatusBadRequest, "malformed JSON body")
		return
	}
	t, err := s.Create(in.Title)
	if err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusCreated, t)
}

// deleteTask removes a task, mapping ErrNotFound to 404 and success to 204.
func deleteTask(w http.ResponseWriter, s svc, id string) {
	if err := s.Delete(id); err != nil {
		respondTask(w, domain.Task{}, err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// respondTask writes a 200 task on success or maps a service error to a status:
// ErrNotFound -> 404, anything else -> 400.
func respondTask(w http.ResponseWriter, t domain.Task, err error) {
	if err != nil {
		if errors.Is(err, service.ErrNotFound) {
			writeError(w, http.StatusNotFound, "task not found")
			return
		}
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, t)
}

// writeJSON serializes payload as JSON with the given status code.
func writeJSON(w http.ResponseWriter, status int, payload any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(payload)
}

// writeError emits a JSON {"error":msg} body with the given status code.
func writeError(w http.ResponseWriter, status int, msg string) {
	writeJSON(w, status, map[string]string{"error": msg})
}
