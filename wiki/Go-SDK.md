# Go SDK

The A1 Go SDK provides a typed HTTP client for the A1 gateway. No Rust toolchain required — it communicates over HTTP/JSON. Supports Go 1.21+, generics, and standard `net/http` patterns.

---

## Installation

```bash
go get github.com/dyologician/a1/sdk/go/a1/kya
```

---

## Start the gateway

```bash
git clone https://github.com/dyologician/A1
cd A1
./setup.sh
```

---

## Core client

```go
package main

import (
    "context"
    "fmt"
    "log"

    a1 "github.com/dyologician/a1/sdk/go/a1/kya"
)

func main() {
    client := a1.New("http://localhost:8080")

    result, err := client.Authorize(context.Background(), a1.AuthorizeRequest{
        Chain:         signedChain,       // map[string]any from your JSON
        IntentName:    "trade.equity",
        IntentParams:  map[string]any{"symbol": "AAPL", "qty": 100},
        ExecutorPkHex: agentPkHex,
    })
    if err != nil {
        log.Fatal(err)
    }

    fmt.Printf("Authorized: %v, depth: %d\n", result.Authorized, result.ChainDepth)
    fmt.Printf("Fingerprint: %s\n", result.ChainFingerprint)
}
```

### Client options

```go
client := a1.New("http://localhost:8080",
    a1.WithTimeout(5 * time.Second),
    a1.WithHeader("Authorization", "Bearer "+os.Getenv("A1_ADMIN_SECRET")),
    a1.WithHTTPClient(myCustomHTTPClient),
)
```

---

## Guard function (`WithPassport`)

Wraps any typed function with an authorization gate using generics.

```go
import a1 "github.com/dyologician/a1/sdk/go/a1/kya"

type TradeInput struct {
    Symbol string
    Qty    int
}

type TradeOutput struct {
    Status string
}

func executeTrade(ctx context.Context, input TradeInput) (TradeOutput, error) {
    // This only runs after authorization succeeds
    return TradeOutput{Status: "filled"}, nil
}

// Wrap with A1 authorization
guarded := a1.WithPassport(client, "trade.equity", executeTrade)

// Call — authorization is checked before executeTrade runs
result, err := guarded(ctx, TradeInput{Symbol: "AAPL", Qty: 100}, chain, agentPkHex)
```

---

## Passport client

Manages passport files and builds delegation chains automatically.

```go
import a1 "github.com/dyologician/a1/sdk/go/a1/kya"

passport, err := a1.NewPassportClient(a1.PassportClientOptions{
    GatewayURL:   "http://localhost:8080",
    PassportPath: "./passport.json",
    AdminSecret:  os.Getenv("A1_ADMIN_SECRET"),
})
if err != nil {
    log.Fatal(err)
}

// Authorize with automatic chain building
receipt, err := passport.Authorize(ctx, a1.AuthorizeOptions{
    IntentName:    "trade.equity",
    ExecutorPkHex: agentPkHex,
})

// Issue a sub-delegation cert for a sub-agent
subCert, err := passport.IssueSub(ctx, a1.IssueSubOptions{
    DelegatePkHex: subAgentPkHex,
    Capabilities:  []string{"trade.equity"},
    TTLSeconds:    3600,
})
```

---

## Batch authorization

```go
results, err := client.AuthorizeBatch(ctx, a1.AuthorizeBatchRequest{
    Chain:         signedChain,
    ExecutorPkHex: agentPkHex,
    Intents: []a1.IntentSpec{
        {IntentName: "trade.equity", IntentParams: map[string]any{"symbol": "AAPL"}},
        {IntentName: "portfolio.read"},
    },
})
```

---

## Admin operations

```go
// Issue a delegation cert (requires admin secret)
cert, err := client.IssueCert(ctx, a1.IssueCertRequest{
    DelegatePkHex: agentPkHex,
    Capabilities:  []string{"trade.equity"},
    TTLSeconds:    3600,
    Namespace:     "trading-bot",
})

// Revoke a cert by fingerprint (requires admin secret)
err = client.RevokeCert(ctx, certFingerprint)

// Check gateway health
health, err := client.Health(ctx)
fmt.Printf("Status: %s\n", health.Status)
```

---

## Error handling

```go
import (
    "errors"
    a1 "github.com/dyologician/a1/sdk/go/a1/kya"
)

result, err := client.Authorize(ctx, req)
if err != nil {
    var authErr *a1.AuthorizationError
    if errors.As(err, &authErr) {
        // Authorization denied: expired cert, scope violation, replay, etc.
        fmt.Printf("Denied: %s (code: %s)\n", authErr.Reason, authErr.ErrorCode)
    } else {
        // Network or gateway error
        fmt.Printf("Gateway error: %v\n", err)
    }
}
```

---

## Full API reference

### `Client`

```go
a1.New(baseURL string, opts ...Option) *Client

client.Authorize(ctx, AuthorizeRequest) (AuthorizeResult, error)
client.AuthorizeBatch(ctx, AuthorizeBatchRequest) (AuthorizeBatchResult, error)
client.IssueCert(ctx, IssueCertRequest) (IssueCertResult, error)
client.RevokeCert(ctx, fingerprint string) error
client.Health(ctx) (HealthResult, error)
```

### `AuthorizeRequest`

```go
type AuthorizeRequest struct {
    Chain          SignedChain        // map[string]any
    IntentName     string
    IntentParams   map[string]any
    ExecutorPkHex  string
    IdempotencyKey string            // optional, for deduplication
}
```

### `AuthorizeResult`

```go
type AuthorizeResult struct {
    Authorized       bool
    ChainDepth       int
    ChainFingerprint string
    Namespace        string
    CapabilityMask   string
}
```

### `PassportReceipt`

```go
type PassportReceipt struct {
    PassportNamespace      string
    FingerprintHex         string
    CapabilityMaskHex      string
    NarrowingCommitmentHex string
    ChainDepth             int
}
```

### `AuthorizationError`

```go
type AuthorizationError struct {
    Reason    string
    ErrorCode string
    Status    int
}
```

---

## Running tests

```bash
cd sdk/go
go test -v ./...
go test -race ./...
go vet ./...
```

---

*Source: `sdk/go/kya/` · [Back to wiki home](Home)*
