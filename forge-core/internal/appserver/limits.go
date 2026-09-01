package appserver

import (
	"net"
	"net/http"
	"sync"
)

const (
	maxServerConnections = 64
	maxInFlightRequests  = 32
)

type connectionLimitListener struct {
	net.Listener
	slots chan struct{}
}

func limitConnections(listener net.Listener, maximum int) net.Listener {
	return &connectionLimitListener{Listener: listener, slots: make(chan struct{}, maximum)}
}

func (listener *connectionLimitListener) Accept() (net.Conn, error) {
	for {
		connection, err := listener.Listener.Accept()
		if err != nil {
			return nil, err
		}
		select {
		case listener.slots <- struct{}{}:
			return &limitedConnection{Conn: connection, release: listener.release}, nil
		default:
			_ = connection.Close()
		}
	}
}

func (listener *connectionLimitListener) release() {
	<-listener.slots
}

type limitedConnection struct {
	net.Conn
	once    sync.Once
	release func()
}

func (connection *limitedConnection) Close() error {
	err := connection.Conn.Close()
	connection.once.Do(connection.release)
	return err
}

type requestLimitHandler struct {
	next  http.Handler
	slots chan struct{}
}

func limitRequests(next http.Handler, maximum int) http.Handler {
	return &requestLimitHandler{next: next, slots: make(chan struct{}, maximum)}
}

func (handler *requestLimitHandler) ServeHTTP(writer http.ResponseWriter, request *http.Request) {
	select {
	case handler.slots <- struct{}{}:
		defer func() { <-handler.slots }()
		handler.next.ServeHTTP(writer, request)
	default:
		writeJSON(writer, request, http.StatusServiceUnavailable, overloadedBody)
	}
}
