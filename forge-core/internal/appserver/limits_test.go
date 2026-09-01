package appserver

import (
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestRequestLimitRejectsExcessInFlightRequest(t *testing.T) {
	entered := make(chan struct{}, 1)
	release := make(chan struct{})
	handler := limitRequests(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		entered <- struct{}{}
		<-release
		writer.WriteHeader(http.StatusNoContent)
	}), 1)
	first := httptest.NewRecorder()
	done := make(chan struct{})
	go func() {
		handler.ServeHTTP(first, httptest.NewRequest(http.MethodGet, "http://127.0.0.1/", nil))
		close(done)
	}()
	<-entered
	second := httptest.NewRecorder()
	handler.ServeHTTP(second, httptest.NewRequest(http.MethodGet, "http://127.0.0.1/", nil))
	close(release)
	<-done
	if first.Code != http.StatusNoContent || second.Code != http.StatusServiceUnavailable {
		t.Fatalf("limited statuses = %d, %d", first.Code, second.Code)
	}
}

func TestConnectionLimitClosesExcessConnectionAndReusesSlot(t *testing.T) {
	upstream := &channelListener{connections: make(chan net.Conn, 3)}
	limited := limitConnections(upstream, 1)
	firstServer, firstClient := net.Pipe()
	upstream.connections <- firstServer
	first, err := limited.Accept()
	if err != nil {
		t.Fatal(err)
	}
	secondServer, secondClient := net.Pipe()
	upstream.connections <- secondServer
	accepted := make(chan net.Conn, 1)
	go func() {
		connection, _ := limited.Accept()
		accepted <- connection
	}()
	assertConnectionClosed(t, secondClient)
	_ = first.Close()
	_ = firstClient.Close()
	thirdServer, thirdClient := net.Pipe()
	upstream.connections <- thirdServer
	select {
	case third := <-accepted:
		_ = third.Close()
	case <-time.After(time.Second):
		t.Fatal("connection slot was not reusable")
	}
	_ = thirdClient.Close()
}

func assertConnectionClosed(t *testing.T, connection net.Conn) {
	t.Helper()
	defer func() { _ = connection.Close() }()
	_ = connection.SetReadDeadline(time.Now().Add(time.Second))
	buffer := make([]byte, 1)
	if _, err := connection.Read(buffer); err != io.EOF {
		t.Fatalf("excess connection read error = %v", err)
	}
}

type channelListener struct {
	connections chan net.Conn
}

func (listener *channelListener) Accept() (net.Conn, error) {
	return <-listener.connections, nil
}

func (*channelListener) Close() error { return nil }

func (*channelListener) Addr() net.Addr { return testAddress("local") }

type testAddress string

func (address testAddress) Network() string { return "test" }

func (address testAddress) String() string { return string(address) }
