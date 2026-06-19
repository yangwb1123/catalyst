// Command taskd is the composition root for the in-memory task service.
//
// It is the ONLY place that wires the concrete layers together: it constructs
// the in-memory store, injects it into the service, builds the HTTP handler and
// serves. Every layer below depends inward only; main depends on all of them.
package main

import (
	"log"
	"net/http"
	"os"

	"example/taskd/internal/httpapi"
	"example/taskd/internal/service"
	"example/taskd/internal/store"
)

func main() {
	addr := os.Getenv("TASKD_ADDR")
	if addr == "" {
		addr = ":8080"
	}
	handler := httpapi.New(service.New(store.NewMemory()))
	log.Printf("taskd listening on %s", addr)
	if err := http.ListenAndServe(addr, handler); err != nil {
		log.Fatalf("taskd: %v", err)
	}
}
