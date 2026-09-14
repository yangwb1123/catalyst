package authn

import (
	"errors"
	"io"
	"net/http"
)

var errRemoteBodyTooLarge = errors.New("authn: remote authentication response exceeds configured maximum")

// safeRemoteClient bounds remote auth responses and rejects redirects. The
// same policy protects both JWKS retrieval and per-request introspection.
func safeRemoteClient(source *http.Client, maxBytes int64) *http.Client {
	client := &http.Client{
		Transport: http.DefaultTransport,
		Timeout:   defaultHTTPTimeout,
		CheckRedirect: func(_ *http.Request, _ []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}
	if source != nil {
		*client = *source
		client.Jar = nil
		client.CheckRedirect = func(_ *http.Request, _ []*http.Request) error {
			return http.ErrUseLastResponse
		}
		if client.Timeout <= 0 || client.Timeout > defaultHTTPTimeout {
			client.Timeout = defaultHTTPTimeout
		}
		if client.Transport == nil {
			client.Transport = http.DefaultTransport
		}
	}
	client.Transport = &boundedBodyTransport{base: client.Transport, maxBytes: maxBytes}
	return client
}

type boundedBodyTransport struct {
	base     http.RoundTripper
	maxBytes int64
}

func (t *boundedBodyTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	resp, err := t.base.RoundTrip(req)
	if err != nil {
		return nil, err
	}
	if resp.Body == nil {
		return resp, nil
	}
	if resp.ContentLength > t.maxBytes {
		_ = resp.Body.Close()
		return nil, errRemoteBodyTooLarge
	}
	resp.Body = &boundedReadCloser{body: resp.Body, remaining: t.maxBytes}
	return resp, nil
}

type boundedReadCloser struct {
	body      io.ReadCloser
	remaining int64
	tooLarge  bool
}

func (b *boundedReadCloser) Read(p []byte) (int, error) {
	if len(p) == 0 {
		return 0, nil
	}
	if b.tooLarge {
		return 0, errRemoteBodyTooLarge
	}
	if b.remaining > 0 {
		if int64(len(p)) > b.remaining {
			p = p[:b.remaining]
		}
		n, err := b.body.Read(p)
		b.remaining -= int64(n)
		return n, err
	}
	var probe [1]byte
	n, err := b.body.Read(probe[:])
	if n > 0 {
		b.tooLarge = true
		return 0, errRemoteBodyTooLarge
	}
	return 0, err
}

func (b *boundedReadCloser) Close() error { return b.body.Close() }
