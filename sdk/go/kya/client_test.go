package a1

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

// fixture helpers

func okServer(t *testing.T, path string, response any) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != path {
			t.Errorf("unexpected path: got %s, want %s", r.URL.Path, path)
			http.Error(w, "not found", http.StatusNotFound)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(response)
	}))
}

func errServer(t *testing.T, status int, code string) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(status)
		_ = json.NewEncoder(w).Encode(map[string]string{
			"error":      "rejected",
			"error_code": code,
		})
	}))
}

// ── New ───────────────────────────────────────────────────────────────────────

func TestNew_defaults(t *testing.T) {
	c := New("http://localhost:8080")
	if c.base != "http://localhost:8080" {
		t.Fatalf("base: got %q", c.base)
	}
	if c.http.Timeout != 10*time.Second {
		t.Fatalf("default timeout: got %v", c.http.Timeout)
	}
}

func TestNew_WithTimeout(t *testing.T) {
	c := New("http://localhost", WithTimeout(5*time.Second))
	if c.http.Timeout != 5*time.Second {
		t.Fatalf("timeout: got %v", c.http.Timeout)
	}
}

func TestNew_WithHeader(t *testing.T) {
	c := New("http://localhost", WithHeader("Authorization", "Bearer token"))
	if c.headers["Authorization"] != "Bearer token" {
		t.Fatalf("header: got %q", c.headers["Authorization"])
	}
}

func TestNew_WithHTTPClient(t *testing.T) {
	inner := &http.Client{Timeout: 3 * time.Second}
	c := New("http://localhost", WithHTTPClient(inner))
	if c.http != inner {
		t.Fatal("custom http client not stored")
	}
}

// ── WellKnown ─────────────────────────────────────────────────────────────────

func TestWellKnown_success(t *testing.T) {
	payload := WellKnownConfig{
		Issuer:        "https://example.com",
		A1Version:    "2.0.0",
		SupportedAlgorithms: []string{"ed25519"},
	}
	srv := okServer(t, "/.well-known/a1-configuration", payload)
	defer srv.Close()

	c := New(srv.URL)
	cfg, err := c.WellKnown(context.Background())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if cfg.Issuer != "https://example.com" {
		t.Fatalf("issuer: got %q", cfg.Issuer)
	}
	if cfg.A1Version != "2.0.0" {
		t.Fatalf("version: got %q", cfg.A1Version)
	}
}

// ── Authorize ─────────────────────────────────────────────────────────────────

func TestAuthorize_authorized(t *testing.T) {
	payload := AuthorizeResponse{
		Authorized:       true,
		ChainDepth:       1,
		ChainFingerprint: "aabbcc",
		VerifiedAtUnix:   1_700_000_000,
	}
	srv := okServer(t, "/v1/authorize", payload)
	defer srv.Close()

	c := New(srv.URL)
	resp, err := c.Authorize(context.Background(), AuthorizeRequest{
		Chain:         SignedChain{},
		IntentName:    "trade.equity",
		ExecutorPkHex: "aa",
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !resp.Authorized {
		t.Fatal("expected authorized=true")
	}
	if resp.ChainDepth != 1 {
		t.Fatalf("chain_depth: got %d", resp.ChainDepth)
	}
}

func TestAuthorize_gateway_error(t *testing.T) {
	srv := errServer(t, http.StatusForbidden, "CHAIN_EXPIRED")
	defer srv.Close()

	c := New(srv.URL)
	_, err := c.Authorize(context.Background(), AuthorizeRequest{IntentName: "trade.equity"})
	if err == nil {
		t.Fatal("expected an error")
	}
	a1Err, ok := err.(*A1Error)
	if !ok {
		t.Fatalf("expected *A1Error, got %T", err)
	}
	if a1Err.ErrorCode != "CHAIN_EXPIRED" {
		t.Fatalf("error_code: got %q", a1Err.ErrorCode)
	}
	if a1Err.Status != http.StatusForbidden {
		t.Fatalf("status: got %d", a1Err.Status)
	}
}

// ── AuthorizeBatch ────────────────────────────────────────────────────────────

func TestAuthorizeBatch_all_authorized(t *testing.T) {
	payload := BatchAuthorizeResponse{
		AllAuthorized:   true,
		AuthorizedCount: 2,
		TotalCount:      2,
		Results: []BatchItem{
			{IntentName: "query.portfolio", Authorized: true},
			{IntentName: "trade.equity", Authorized: true},
		},
	}
	srv := okServer(t, "/v1/authorize/batch", payload)
	defer srv.Close()

	c := New(srv.URL)
	resp, err := c.AuthorizeBatch(context.Background(), BatchAuthorizeRequest{
		ExecutorPkHex: "aa",
		Intents: []BatchIntentRequest{
			{Name: "query.portfolio"},
			{Name: "trade.equity"},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !resp.AllAuthorized {
		t.Fatal("expected all_authorized=true")
	}
	if len(resp.Results) != 2 {
		t.Fatalf("results len: got %d", len(resp.Results))
	}
}

// ── Revoke ────────────────────────────────────────────────────────────────────

func TestRevokeCert_success(t *testing.T) {
	srv := okServer(t, "/v1/cert/revoke", map[string]any{})
	defer srv.Close()

	c := New(srv.URL)
	if err := c.RevokeCert(context.Background(), "aabbcc"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestRevokeCertsBatch_success(t *testing.T) {
	payload := RevokeBatchResponse{RevokedCount: 2, Failed: []string{}}
	srv := okServer(t, "/v1/cert/revoke-batch", payload)
	defer srv.Close()

	c := New(srv.URL)
	resp, err := c.RevokeCertsBatch(context.Background(), []string{"aabb", "ccdd"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.RevokedCount != 2 {
		t.Fatalf("revoked_count: got %d", resp.RevokedCount)
	}
}

// ── InspectCert ───────────────────────────────────────────────────────────────

func TestInspectCert_not_revoked(t *testing.T) {
	payload := CertStatus{Fingerprint: "aabb", Revoked: false}
	srv := okServer(t, "/v1/cert/aabb", payload)
	defer srv.Close()

	c := New(srv.URL)
	status, err := c.InspectCert(context.Background(), "aabb")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if status.Revoked {
		t.Fatal("expected revoked=false")
	}
}

// ── A1Error ─────────────────────────────────────────────────────────────────

func TestA1Error_message_format(t *testing.T) {
	err := &A1Error{
		Message:   "forbidden",
		ErrorCode: "CERT_REVOKED",
		Status:    403,
	}
	if err.Error() == "" {
		t.Fatal("Error() should return non-empty string")
	}
	err2 := &A1Error{Message: "generic", Status: 500}
	if err2.Error() == "" {
		t.Fatal("Error() should return non-empty string without code")
	}
}

// ── non-JSON gateway response ─────────────────────────────────────────────────

func TestAuthorize_nonJSON_500(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusBadGateway)
		_, _ = w.Write([]byte("<html>502 Bad Gateway</html>"))
	}))
	defer srv.Close()

	c := New(srv.URL)
	_, err := c.Authorize(context.Background(), AuthorizeRequest{IntentName: "trade.equity"})
	if err == nil {
		t.Fatal("expected an error for 502")
	}
	a1Err, ok := err.(*A1Error)
	if !ok {
		t.Fatalf("expected *A1Error, got %T", err)
	}
	if a1Err.Status != http.StatusBadGateway {
		t.Fatalf("status: got %d", a1Err.Status)
	}
}
