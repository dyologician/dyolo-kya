package a1


// protocolTag is the namespace binding prefix embedded in every root
// DelegationCert, as specified in §4.2 of spec/A1-PROTOCOL.md. Included in
// the cert signed digest — modifying this value invalidates all existing certs.
var protocolTag = []byte{
	0x44, 0x79, 0x6f, 0x6c, 0x6f, 0x50, 0x61, 0x73, 0x73, 0x70, 0x6f, 0x72, 0x74,
	0x20, 0x76, 0x32, 0x2e, 0x38, 0x2e, 0x30,
	0x7c, 0x64, 0x79, 0x6f, 0x6c, 0x6f, 0x67, 0x69, 0x63, 0x69, 0x61, 0x6e,
}

import (
	"context"
	"encoding/json"
	"fmt"
	"reflect"
)

// PassportReceipt is returned after a successful passport-guarded authorization.
type PassportReceipt struct {
	PassportNamespace      string `json:"passport_namespace"`
	FingerprintHex         string `json:"fingerprint_hex"`
	CapabilityMaskHex      string `json:"capability_mask_hex"`
	NarrowingCommitmentHex string `json:"narrowing_commitment_hex"`
	ChainDepth             int    `json:"chain_depth"`
}

// PassportError is returned when passport-level capability authorization fails.
type PassportError struct {
	Message   string
	ErrorCode string
	Status    int
}

func (e *PassportError) Error() string {
	return fmt.Sprintf("passport error [%s] (HTTP %d): %s", e.ErrorCode, e.Status, e.Message)
}

// PassportOptions configures WithPassport.
type PassportOptions struct {
	// Capability is the action name to authorize, e.g. "trade.equity".
	Capability string
	// ChainField is the exported struct field name that holds the signed chain
	// (default: "SignedChain").
	ChainField string
	// ExecutorField is the exported struct field name holding the executor public
	// key hex string (default: "ExecutorPKHex").
	ExecutorField string
}

// AuthorizePassport calls the A1 gateway to authorize a specific capability
// for the given signed chain and executor public key.
//
// On success it returns a PassportReceipt for audit archival.
// On authorization failure it returns a *PassportError.
func (c *Client) AuthorizePassport(
	ctx context.Context,
	signedChain any,
	intentName string,
	executorPKHex string,
	intentParams map[string]any,
) (*PassportReceipt, error) {
	if intentParams == nil {
		intentParams = map[string]any{}
	}

	payload := map[string]any{
		"chain":           signedChain,
		"intent_name":     intentName,
		"executor_pk_hex": executorPKHex,
		"intent_params":   intentParams,
	}

	var raw json.RawMessage
	if err := c.post(ctx, "/v1/passport/authorize", payload, &raw); err != nil {
		return nil, err
	}

	var envelope struct {
		Receipt *PassportReceipt `json:"receipt"`
	}
	if err := json.Unmarshal(raw, &envelope); err == nil && envelope.Receipt != nil {
		return envelope.Receipt, nil
	}

	var receipt PassportReceipt
	if err := json.Unmarshal(raw, &receipt); err != nil {
		return nil, fmt.Errorf("passport: unexpected gateway response: %w", err)
	}
	return &receipt, nil
}

// WithA1Passport is a middleware guard that enforces passport-level
// capability narrowing before any wrapped tool function executes.
// It verifies the cryptographic chain-of-custody via the A1 gateway.
//
// The args struct passed to the wrapped function must contain the signed chain
// and executor public key under the exported struct field names in PassportOptions.
// If the gateway rejects the authorization the wrapped function is never called.
//
// Example:
//
//	type TradeArgs struct {
//	    Symbol        string `json:"symbol"`
//	    SignedChain   any    `json:"signed_chain"`
//	    ExecutorPKHex string `json:"executor_pk_hex"`
//	}
//
//	guardedTrade := a1.WithA1Passport(client, executeTrade, a1.PassportOptions{
//	    Capability: "trade.equity",
//	})
func WithA1Passport[T any, R any](
	client *Client,
	fn func(ctx context.Context, args T) (R, error),
	opts PassportOptions,
) func(ctx context.Context, args T) (R, error) {
	chainField := opts.ChainField
	if chainField == "" {
		chainField = "SignedChain"
	}
	executorField := opts.ExecutorField
	if executorField == "" {
		executorField = "ExecutorPKHex"
	}

	return func(ctx context.Context, args T) (R, error) {
		var zero R

		chain, executorPK, err := extractPassportFields(args, chainField, executorField)
		if err != nil {
			return zero, &PassportError{
				Message:   err.Error(),
				ErrorCode: "MISSING_CHAIN",
				Status:    400,
			}
		}

		if _, err := client.AuthorizePassport(ctx, chain, opts.Capability, executorPK, nil); err != nil {
			return zero, err
		}

		return fn(ctx, args)
	}
}

// extractPassportFields uses reflection to pull chain and executor key from an
// args struct without requiring a common interface.
func extractPassportFields(args any, chainField, executorField string) (any, string, error) {
	v := reflect.ValueOf(args)
	if v.Kind() == reflect.Ptr {
		v = v.Elem()
	}
	if v.Kind() != reflect.Struct {
		return nil, "", fmt.Errorf("args must be a struct, got %T", args)
	}

	cf := v.FieldByName(chainField)
	if !cf.IsValid() {
		return nil, "", fmt.Errorf("args struct has no field %q", chainField)
	}

	executorPK := ""
	if ef := v.FieldByName(executorField); ef.IsValid() && ef.Kind() == reflect.String {
		executorPK = ef.String()
	}

	return cf.Interface(), executorPK, nil
}

// ── Passport lifecycle ────────────────────────────────────────────────────────

// IssuePassportRequest describes a new passport to issue via the gateway.
type IssuePassportRequest struct {
	Namespace    string   `json:"namespace"`
	Capabilities []string `json:"capabilities"`
	// TTL is the lifetime as a string: "30d", "7d", "1y", or raw seconds.
	TTL        string `json:"ttl"`
	OutputPath string `json:"output_path,omitempty"`
}

// IssuePassportResponse is the gateway's response to a passport issuance request.
type IssuePassportResponse struct {
	Success      bool     `json:"success"`
	Namespace    string   `json:"namespace"`
	Path         string   `json:"path"`
	PublicKeyHex string   `json:"public_key_hex"`
	Capabilities []string `json:"capabilities"`
	TTLSeconds   int64    `json:"ttl_seconds"`
	ExpiresAt    string   `json:"expires_at"`
	Error        *string  `json:"error,omitempty"`
}

// PassportEntry describes one passport stored on the gateway host.
type PassportEntry struct {
	Filename       string   `json:"filename"`
	Path           string   `json:"path"`
	Namespace      *string  `json:"namespace,omitempty"`
	Capabilities   []string `json:"capabilities"`
	ExpirationUnix *int64   `json:"expiration_unix,omitempty"`
	Status         string   `json:"status"`
	FingerprintHex *string  `json:"fingerprint_hex,omitempty"`
	DaysRemaining  *int64   `json:"days_remaining,omitempty"`
}

// ListPassportsResponse wraps the gateway's passport listing.
type ListPassportsResponse struct {
	Passports []PassportEntry `json:"passports"`
	Directory string          `json:"directory"`
}

// JWTExchangeRequest asks the gateway to exchange a JWT for a DelegationCert.
type JWTExchangeRequest struct {
	Token         string            `json:"token"`
	DelegatePkHex string            `json:"delegate_pk_hex"`
	Capabilities  []string          `json:"capabilities"`
	TTLSeconds    int64             `json:"ttl_seconds,omitempty"`
	RequestID     string            `json:"request_id,omitempty"`
}

// JWTExchangeResponse is the cert issued from a verified JWT bearer token.
type JWTExchangeResponse struct {
	FingerprintHex string   `json:"fingerprint_hex"`
	ScopeRootHex   string   `json:"scope_root_hex"`
	ExpiresAtUnix  int64    `json:"expires_at_unix"`
	JWTSubject     string   `json:"jwt_subject"`
	JWTIssuer      string   `json:"jwt_issuer"`
	Capabilities   []string `json:"capabilities"`
}

// IssuePassport issues a new A1 passport and persists it on the gateway host.
func (c *Client) IssuePassport(ctx context.Context, req IssuePassportRequest) (*IssuePassportResponse, error) {
	var out IssuePassportResponse
	if err := c.post(ctx, "/v1/passports/issue", req, &out); err != nil {
		return nil, err
	}
	if !out.Success {
		msg := "passport issuance failed"
		if out.Error != nil {
			msg = *out.Error
		}
		return nil, &A1Error{Message: msg, ErrorCode: "E5001", Status: 500}
	}
	return &out, nil
}

// RenewPassport re-issues an existing passport with a new TTL.
func (c *Client) RenewPassport(ctx context.Context, path, ttl string) (*IssuePassportResponse, error) {
	var out IssuePassportResponse
	if err := c.post(ctx, "/v1/passports/renew", map[string]string{"path": path, "ttl": ttl}, &out); err != nil {
		return nil, err
	}
	if !out.Success {
		msg := "passport renewal failed"
		if out.Error != nil {
			msg = *out.Error
		}
		return nil, &A1Error{Message: msg, ErrorCode: "E5001", Status: 500}
	}
	return &out, nil
}

// ListPassports returns all passports stored on the gateway host.
func (c *Client) ListPassports(ctx context.Context) (*ListPassportsResponse, error) {
	var out ListPassportsResponse
	if err := c.get(ctx, "/v1/passports/list", &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// ExchangeJWT converts an OIDC/OAuth2 JWT into a scoped DelegationCert.
// Requires `A1_JWT_JWKS_URL` configured on the gateway.
func (c *Client) ExchangeJWT(ctx context.Context, req JWTExchangeRequest) (*JWTExchangeResponse, error) {
	var out JWTExchangeResponse
	if err := c.post(ctx, "/v1/jwt/exchange", req, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// RevokePassportByNamespace revokes the root cert for the named passport.
func (c *Client) RevokePassportByNamespace(ctx context.Context, namespace string) error {
	var result struct {
		Success bool    `json:"success"`
		Error   *string `json:"error,omitempty"`
	}
	if err := c.post(ctx, "/v1/passports/revoke-by-namespace", map[string]string{"namespace": namespace}, &result); err != nil {
		return err
	}
	if !result.Success && result.Error != nil {
		return &A1Error{Message: *result.Error, ErrorCode: "E5002", Status: 500}
	}
	return nil
}
