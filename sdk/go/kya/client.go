// Package a1 is the Go client for the a1 A1 Passport gateway.
//
// The a1 protocol provides cryptographic chain-of-custody for recursive
// AI agent delegation. This client wraps the gateway REST API so any Go service
// can verify agent authorization without running Rust code.
//
// Usage:
//
//	c := a1.New("http://localhost:8080")
//	result, err := c.Authorize(ctx, a1.AuthorizeRequest{...})
package a1

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Client talks to the a1 gateway over HTTP.
type Client struct {
	base    string
	http    *http.Client
	headers map[string]string
}

// Option configures a Client.
type Option func(*Client)

// WithTimeout overrides the default 10-second HTTP timeout.
func WithTimeout(d time.Duration) Option {
	return func(c *Client) { c.http.Timeout = d }
}

// WithHeader adds a static header to every request (e.g. "Authorization").
func WithHeader(key, value string) Option {
    return func(c *Client) { c.headers[key] = value }
}

// WithHTTPClient overrides the default HTTP client for enterprise proxy/TLS support.
func WithHTTPClient(client *http.Client) Option {
    return func(c *Client) { c.http = client }
}

// New returns a Client pointed at the a1 gateway at baseURL.
func New(baseURL string, opts ...Option) *Client {
	c := &Client{
		base:    baseURL,
		http:    &http.Client{Timeout: 10 * time.Second},
		headers: map[string]string{
			"User-Agent": "a1-sdk-go/dyolo_v2.8.0",
		},
	}
	for _, o := range opts {
		o(c)
	}
	return c
}

// ── Wire types ────────────────────────────────────────────────────────────────

// DalekBytes correctly marshals/unmarshals a JSON array of integers
// into a Go byte slice, avoiding Go's default base64 encoding for []byte
// which crashes when reading Dalek's default serialization.
type DalekBytes []byte

func (b *DalekBytes) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		return nil
	}
	var ints []int
	if err := json.Unmarshal(data, &ints); err != nil {
		return err
	}
	res := make([]byte, len(ints))
	for i, v := range ints {
		res[i] = byte(v)
	}
	*b = res
	return nil
}

func (b DalekBytes) MarshalJSON() ([]byte, error) {
	if b == nil {
		return []byte("null"), nil
	}
	ints := make([]int, len(b))
	for i, v := range b {
		ints[i] = int(v)
	}
	return json.Marshal(ints)
}

// DelegationCert is a single hop in a delegation chain.
type DelegationCert struct {
	Version     int        `json:"version"`
	DelegatePk  DalekBytes `json:"delegate_pk"`
	ScopeRoot   DalekBytes `json:"scope_root"`
	NotBefore   int64      `json:"not_before"`
	ExpiresAt   int64      `json:"expires_at"`
	MaxDepth    int        `json:"max_depth"`
	Signature   DalekBytes `json:"signature"`
}

// SignedChain is a delegation chain ready for wire transport.
type SignedChain struct {
	Certs    []DelegationCert `json:"certs"`
	RootPk   DalekBytes       `json:"root_pk"`
	Mac      DalekBytes       `json:"mac,omitempty"`
}

// VerifiedToken is an HMAC-signed receipt issued by the gateway when
// authorize is called with ReturnToken: true.
type VerifiedToken struct {
	ReceiptJSON string     `json:"receipt_json"`
	Mac         DalekBytes `json:"mac"`
}

// ── Request / response types ──────────────────────────────────────────────────

// IssueCertRequest describes a certificate to be issued by the gateway.
type IssueCertRequest struct {
	DelegatePkHex string                 `json:"delegate_pk_hex"`
	Intents       []IntentSpec           `json:"intents"`
	TTLSeconds    int64                  `json:"ttl_seconds,omitempty"`
	MaxDepth      int                    `json:"max_depth,omitempty"`
	Extensions    map[string]interface{} `json:"extensions,omitempty"`
}

// IntentSpec is a named action with optional parameters.
type IntentSpec struct {
	Name   string            `json:"name"`
	Params map[string]string `json:"params,omitempty"`
}

// IssueCertResponse is the gateway's response to a cert issuance request.
type IssueCertResponse struct {
	FingerprintHex string `json:"fingerprint_hex"`
	ScopeRootHex   string `json:"scope_root_hex"`
}

// AuthorizeRequest asks the gateway to verify an agent's authorization.
type AuthorizeRequest struct {
	Chain         SignedChain       `json:"chain"`
	IntentName    string            `json:"intent_name"`
	IntentParams  map[string]string `json:"intent_params,omitempty"`
	ExecutorPkHex string            `json:"executor_pk_hex"`
	ReturnToken   bool              `json:"return_token,omitempty"`
	RequestID     string            `json:"request_id,omitempty"`
}

// AuthorizeResponse is the gateway's verdict on an authorization request.
type AuthorizeResponse struct {
	Authorized       bool            `json:"authorized"`
	ChainDepth       int             `json:"chain_depth"`
	ChainFingerprint string          `json:"chain_fingerprint"`
	VerifiedAtUnix   int64           `json:"verified_at_unix"`
	ErrorCode        *string         `json:"error_code,omitempty"`
	Receipt          PassportReceipt `json:"receipt"`
	Token            *VerifiedToken  `json:"token,omitempty"`
}

// BatchIntentRequest is a single intent within a batch authorization call.
type BatchIntentRequest struct {
	Name   string            `json:"name"`
	Params map[string]string `json:"params,omitempty"`
}

// BatchAuthorizeRequest asks the gateway to verify multiple intents atomically.
type BatchAuthorizeRequest struct {
	Chain         SignedChain          `json:"chain"`
	ExecutorPkHex string               `json:"executor_pk_hex"`
	Intents       []BatchIntentRequest `json:"intents"`
}

// BatchItem is the result for one intent in a batch.
type BatchItem struct {
	IntentName       string `json:"intent_name"`
	Authorized       bool   `json:"authorized"`
	ChainFingerprint string `json:"chain_fingerprint,omitempty"`
	Error            string `json:"error,omitempty"`
	ErrorCode        string `json:"error_code,omitempty"`
}

// BatchAuthorizeResponse is the gateway's verdict on a batch authorization.
type BatchAuthorizeResponse struct {
	AllAuthorized   bool        `json:"all_authorized"`
	AuthorizedCount int         `json:"authorized_count"`
	TotalCount      int         `json:"total_count"`
	Results         []BatchItem `json:"results"`
}

// RevokeRequest revokes a single cert by fingerprint.
type RevokeRequest struct {
	FingerprintHex string `json:"fingerprint_hex"`
}

// RevokeBatchRequest revokes multiple certs in a single call.
type RevokeBatchRequest struct {
	Fingerprints []string `json:"fingerprints"`
}

// RevokeBatchResponse is the gateway's response to a batch revocation.
type RevokeBatchResponse struct {
	RevokedCount int      `json:"revoked_count"`
	Failed       []string `json:"failed"`
}

// CertStatus is the response to an inspect request.
type CertStatus struct {
	Fingerprint string `json:"fingerprint"`
	Revoked     bool   `json:"revoked"`
}

// WellKnownConfig is the OIDC-style discovery document for the gateway.
type WellKnownConfig struct {
	Issuer                   string   `json:"issuer"`
	GatewaySigningPkHex      string   `json:"gateway_signing_pk_hex"`
	AuthorizationEndpoint    string   `json:"authorization_endpoint"`
	BatchAuthorizeEndpoint   string   `json:"batch_authorize_endpoint"`
	CertIssuanceEndpoint     string   `json:"cert_issuance_endpoint"`
	CertRevokeEndpoint       string   `json:"cert_revoke_endpoint"`
	CertRevokeBatchEndpoint  string   `json:"cert_revoke_batch_endpoint"`
	TokenVerifyEndpoint      string   `json:"token_verify_endpoint"`
	A1Version               string   `json:"a1_version"`
	SupportedAlgorithms      []string `json:"supported_algorithms"`
}

// VerifyTokenRequest asks the gateway to verify a VerifiedToken MAC.
type VerifyTokenRequest struct {
	Token VerifiedToken `json:"token"`
}

// VerifyTokenResult is the decoded receipt if the MAC verifies.
type VerifyTokenResult struct {
	Valid           bool   `json:"valid"`
	ChainDepth      int    `json:"chain_depth"`
	ChainFingerprint string `json:"chain_fingerprint"`
	VerifiedAtUnix  int64  `json:"verified_at_unix"`
}

// ── API methods ───────────────────────────────────────────────────────────────

// WellKnown returns the gateway's OIDC-style discovery document.
func (c *Client) WellKnown(ctx context.Context) (*WellKnownConfig, error) {
	var out WellKnownConfig
	if err := c.get(ctx, "/.well-known/a1-configuration", &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// IssueCert asks the gateway to issue a signed DelegationCert.
func (c *Client) IssueCert(ctx context.Context, req IssueCertRequest) (*IssueCertResponse, error) {
	var out IssueCertResponse
	if err := c.post(ctx, "/v1/cert/issue", req, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// Authorize verifies that an agent is authorized to execute an intent
// under the given delegation chain.
func (c *Client) Authorize(ctx context.Context, req AuthorizeRequest) (*AuthorizeResponse, error) {
	var out AuthorizeResponse
	if err := c.post(ctx, "/v1/authorize", req, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// AuthorizeBatch verifies multiple intents atomically against a single chain.
// If any intent fails, no nonces are consumed and the full batch is rejected.
func (c *Client) AuthorizeBatch(ctx context.Context, req BatchAuthorizeRequest) (*BatchAuthorizeResponse, error) {
	var out BatchAuthorizeResponse
	if err := c.post(ctx, "/v1/authorize/batch", req, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// RevokeCert revokes a single certificate by its fingerprint.
func (c *Client) RevokeCert(ctx context.Context, fingerprintHex string) error {
	var out struct{}
	return c.post(ctx, "/v1/cert/revoke", RevokeRequest{FingerprintHex: fingerprintHex}, &out)
}

// RevokeCertsBatch revokes multiple certificates in one round-trip.
func (c *Client) RevokeCertsBatch(ctx context.Context, fingerprints []string) (*RevokeBatchResponse, error) {
	var out RevokeBatchResponse
	if err := c.post(ctx, "/v1/cert/revoke-batch", RevokeBatchRequest{Fingerprints: fingerprints}, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// InspectCert returns the revocation status of a certificate.
func (c *Client) InspectCert(ctx context.Context, fingerprintHex string) (*CertStatus, error) {
	var out CertStatus
	if err := c.get(ctx, "/v1/cert/"+fingerprintHex, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// VerifyToken verifies the HMAC on a VerifiedToken returned by Authorize.
func (c *Client) VerifyToken(ctx context.Context, token VerifiedToken) (*VerifyTokenResult, error) {
	var out VerifyTokenResult
	if err := c.post(ctx, "/v1/token/verify", token, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

// ── Error ─────────────────────────────────────────────────────────────────────

// A1Error is returned when the gateway rejects a request.
type A1Error struct {
	Message   string
	ErrorCode string
	Status    int
}

func (e *A1Error) Error() string {
	if e.ErrorCode != "" {
		return fmt.Sprintf("a1 [%s]: %s (HTTP %d)", e.ErrorCode, e.Message, e.Status)
	}
	return fmt.Sprintf("a1: %s (HTTP %d)", e.Message, e.Status)
}

// ── Internal HTTP helpers ─────────────────────────────────────────────────────

func (c *Client) post(ctx context.Context, path string, body, out interface{}) error {
	data, err := json.Marshal(body)
	if err != nil {
		return fmt.Errorf("a1: marshal request: %w", err)
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.base+path, bytes.NewReader(data))
	if err != nil {
		return fmt.Errorf("a1: build request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	for k, v := range c.headers {
		req.Header.Set(k, v)
	}
	return c.do(req, out)
}

func (c *Client) get(ctx context.Context, path string, out interface{}) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.base+path, nil)
	if err != nil {
		return fmt.Errorf("a1: build request: %w", err)
	}
	req.Header.Set("Accept", "application/json")
	for k, v := range c.headers {
		req.Header.Set(k, v)
	}
	return c.do(req, out)
}

func (c *Client) do(req *http.Request, out interface{}) error {
	resp, err := c.http.Do(req)
	if err != nil {
		return fmt.Errorf("a1: http: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("a1: read response: %w", err)
	}

	if resp.StatusCode >= 400 {
		var errBody struct {
			Error     string `json:"error"`
			ErrorCode string `json:"error_code"`
		}
		_ = json.Unmarshal(body, &errBody)
		
		// Fallback for non-JSON or HTML gateway errors (e.g. 502/503 from load balancers)
		if errBody.Error == "" {
			errBody.Error = string(body)
			if errBody.Error == "" {
				errBody.Error = http.StatusText(resp.StatusCode)
			}
		}

		return &A1Error{
			Message:   errBody.Error,
			ErrorCode: errBody.ErrorCode,
			Status:    resp.StatusCode,
		}
	}

	if out != nil {
		if err := json.Unmarshal(body, out); err != nil {
			return fmt.Errorf("a1: decode response: %w", err)
		}
	}
	return nil
}
