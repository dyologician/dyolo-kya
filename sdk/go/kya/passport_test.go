package a1

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// ── Fixtures ──────────────────────────────────────────────────────────────────

var samplePassportReceipt = map[string]any{
	"receipt": map[string]any{
		"passport_namespace":       "test-agent",
		"fingerprint_hex":          "deadbeef" + "deadbeef" + "deadbeef" + "deadbeef" + "deadbeef" + "deadbeef" + "deadbeef" + "deadbeef",
		"capability_mask_hex":      "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff" + "ff",
		"narrowing_commitment_hex": "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab" + "ab",
		"chain_depth":              1,
	},
}

func passportOkServer(t *testing.T) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(samplePassportReceipt)
	}))
}

func passportErrServer(t *testing.T, status int, code string) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(status)
		_ = json.NewEncoder(w).Encode(map[string]string{
			"error":      "authorization denied",
			"error_code": code,
		})
	}))
}

// ── AuthorizePassport ─────────────────────────────────────────────────────────

func TestAuthorizePassport_Success(t *testing.T) {
	srv := passportOkServer(t)
	defer srv.Close()

	client := New(srv.URL)
	receipt, err := client.AuthorizePassport(
		context.Background(),
		map[string]any{"certs": []any{}},
		"trade.equity",
		"aabbcc",
		nil,
	)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receipt.PassportNamespace != "test-agent" {
		t.Errorf("namespace: got %q, want %q", receipt.PassportNamespace, "test-agent")
	}
	if receipt.ChainDepth != 1 {
		t.Errorf("chain_depth: got %d, want 1", receipt.ChainDepth)
	}
}

func TestAuthorizePassport_FlatReceiptFallback(t *testing.T) {
	flat := map[string]any{
		"passport_namespace":       "bot",
		"fingerprint_hex":          "00" + "00" + "00" + "00",
		"capability_mask_hex":      "ff" + "ff",
		"narrowing_commitment_hex": "cc" + "cc",
		"chain_depth":              2,
	}
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(flat)
	}))
	defer srv.Close()

	client := New(srv.URL)
	receipt, err := client.AuthorizePassport(context.Background(), nil, "read.data", "", nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receipt.PassportNamespace != "bot" {
		t.Errorf("namespace: got %q, want %q", receipt.PassportNamespace, "bot")
	}
	if receipt.ChainDepth != 2 {
		t.Errorf("chain_depth: got %d, want 2", receipt.ChainDepth)
	}
}

func TestAuthorizePassport_AuthorizationFailure(t *testing.T) {
	srv := passportErrServer(t, http.StatusForbidden, "PASSPORT_NARROWING_VIOLATION")
	defer srv.Close()

	client := New(srv.URL)
	_, err := client.AuthorizePassport(context.Background(), nil, "trade.equity", "", nil)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	pe, ok := err.(*PassportError)
	if !ok {
		t.Fatalf("expected *PassportError, got %T: %v", err, err)
	}
	if pe.ErrorCode != "PASSPORT_NARROWING_VIOLATION" {
		t.Errorf("error_code: got %q, want %q", pe.ErrorCode, "PASSPORT_NARROWING_VIOLATION")
	}
	if pe.Status != http.StatusForbidden {
		t.Errorf("status: got %d, want %d", pe.Status, http.StatusForbidden)
	}
}

func TestAuthorizePassport_NilIntentParams_Allowed(t *testing.T) {
	srv := passportOkServer(t)
	defer srv.Close()

	client := New(srv.URL)
	_, err := client.AuthorizePassport(context.Background(), nil, "trade.equity", "", nil)
	if err != nil {
		t.Fatalf("nil intent_params should not error: %v", err)
	}
}

func TestAuthorizePassport_WithIntentParams(t *testing.T) {
	var gotBody map[string]any
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewDecoder(r.Body).Decode(&gotBody)
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(samplePassportReceipt)
	}))
	defer srv.Close()

	client := New(srv.URL)
	params := map[string]any{"symbol": "AAPL", "qty": 10}
	_, err := client.AuthorizePassport(context.Background(), nil, "trade.equity", "pk", params)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotBody["intent_name"] != "trade.equity" {
		t.Errorf("intent_name: got %v", gotBody["intent_name"])
	}
	if gotBody["executor_pk_hex"] != "pk" {
		t.Errorf("executor_pk_hex: got %v", gotBody["executor_pk_hex"])
	}
}

// ── WithPassport ──────────────────────────────────────────────────────────────

type tradeArgs struct {
	Symbol        string `json:"symbol"`
	SignedChain   any    `json:"signed_chain"`
	ExecutorPKHex string `json:"executor_pk_hex"`
}

func TestWithPassport_SuccessCallsWrappedFn(t *testing.T) {
	srv := passportOkServer(t)
	defer srv.Close()

	client := New(srv.URL)
	called := false

	guardedFn := WithPassport(client, func(ctx context.Context, args tradeArgs) (string, error) {
		called = true
		return "filled", nil
	}, PassportOptions{Capability: "trade.equity"})

	result, err := guardedFn(context.Background(), tradeArgs{
		Symbol:        "AAPL",
		SignedChain:   map[string]any{"certs": []any{}},
		ExecutorPKHex: "aabb",
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "filled" {
		t.Errorf("result: got %q, want %q", result, "filled")
	}
	if !called {
		t.Error("wrapped function was not called")
	}
}

func TestWithPassport_AuthFailureBlocksWrappedFn(t *testing.T) {
	srv := passportErrServer(t, http.StatusForbidden, "SCOPE_VIOLATION")
	defer srv.Close()

	client := New(srv.URL)
	called := false

	guardedFn := WithPassport(client, func(ctx context.Context, args tradeArgs) (string, error) {
		called = true
		return "should-not-run", nil
	}, PassportOptions{Capability: "trade.equity"})

	_, err := guardedFn(context.Background(), tradeArgs{Symbol: "AAPL"})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if called {
		t.Error("wrapped function must not run when authorization fails")
	}
	pe, ok := err.(*PassportError)
	if !ok {
		t.Fatalf("expected *PassportError, got %T", err)
	}
	if pe.ErrorCode != "SCOPE_VIOLATION" {
		t.Errorf("error_code: got %q", pe.ErrorCode)
	}
}

func TestWithPassport_MissingChainField(t *testing.T) {
	client := New("http://localhost:9999") // no server needed

	type noChainArgs struct {
		Symbol string `json:"symbol"`
	}

	guardedFn := WithPassport(client, func(ctx context.Context, args noChainArgs) (string, error) {
		return "ok", nil
	}, PassportOptions{Capability: "trade.equity"})

	_, err := guardedFn(context.Background(), noChainArgs{Symbol: "AAPL"})
	if err == nil {
		t.Fatal("expected error for missing chain field")
	}
	pe, ok := err.(*PassportError)
	if !ok {
		t.Fatalf("expected *PassportError, got %T: %v", err, err)
	}
	if pe.ErrorCode != "MISSING_CHAIN" {
		t.Errorf("error_code: got %q, want MISSING_CHAIN", pe.ErrorCode)
	}
}

func TestWithPassport_CustomFieldNames(t *testing.T) {
	srv := passportOkServer(t)
	defer srv.Close()

	type customArgs struct {
		MyChain any    `json:"chain"`
		MyPK    string `json:"pk"`
	}

	client := New(srv.URL)
	called := false

	guardedFn := WithPassport(client, func(ctx context.Context, args customArgs) (string, error) {
		called = true
		return "ok", nil
	}, PassportOptions{
		Capability:    "trade.equity",
		ChainField:    "MyChain",
		ExecutorField: "MyPK",
	})

	_, err := guardedFn(context.Background(), customArgs{
		MyChain: map[string]any{},
		MyPK:    "pk",
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !called {
		t.Error("wrapped function was not called")
	}
}

func TestPassportError_ErrorString(t *testing.T) {
	e := &PassportError{Message: "denied", ErrorCode: "SCOPE_VIOLATION", Status: 403}
	s := e.Error()
	if s == "" {
		t.Error("Error() must not return empty string")
	}
}