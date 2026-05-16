# A1 Go SDK

[![Go Reference](https://img.shields.io/badge/go-reference-blue)](https://pkg.go.dev/github.com/dyologician/a1/sdk/go/a1/kya)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/dyologician/a1/blob/main/LICENSE-MIT)

Go SDK for [A1](https://github.com/dyologician/a1) — cryptographic chain-of-custody for recursive AI agent delegation.

A1 gives every AI agent a verifiable passport and produces an independently verifiable receipt for every authorized action. It closes the **Recursive Delegation Gap**: the inability to prove, in a multi-agent delegation chain, which human authorized the action at the end.

No Rust toolchain required — this SDK communicates with the self-hosted A1 gateway over HTTP/JSON.

---

## Requirements

- Go 1.21+
- An A1 gateway running locally or remotely

---

## Installation

```bash
go get github.com/dyologician/a1/sdk/go/a1/kya
```

---

## Quick Start

### 1. Start the gateway

```bash
docker compose up -d   # from the A1 repository root
```

The gateway listens on `http://localhost:8080`.

### 2. Create a client and authorize an action

```go
package main

import (
    "context"
    "fmt"
    "log"

    a1 "github.com/dyologician/a1/sdk/go/a1/kya"
)

func main() {
    client := a1.NewClient("http://localhost:8080", nil)

    result, err := client.Authorize(context.Background(), a1.AuthorizeRequest{
        Chain:         signedChain,
        IntentName:    "trade.equity",
        IntentParams:  map[string]any{"symbol": "AAPL", "qty": 100},
        ExecutorPkHex: agentPkHex,
    })
    if err != nil {
        log.Fatal(err)
    }

    fmt.Printf("Authorized: %v, depth: %d\n", result.Authorized, result.ChainDepth)
}
```

### 3. Guard a function with `WithPassport`

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
    return TradeOutput{Status: "filled"}, nil
}

// Wrap with A1 authorization
guarded := a1.WithPassport(client, "trade.equity", executeTrade)

// Calling guarded verifies authorization first:
result, err := guarded(ctx, TradeInput{Symbol: "AAPL", Qty: 100}, chain, agentPkHex)
```

---

## Passport Client

```go
import a1 "github.com/dyologician/a1/sdk/go/a1/kya"

passport, err := a1.NewPassportClient(a1.PassportClientOptions{
    GatewayURL:    "http://localhost:8080",
    PassportPath:  "./passport.json",
    AdminSecret:   os.Getenv("A1_ADMIN_SECRET"),
})
if err != nil {
    log.Fatal(err)
}

// Authorize with automatic chain building
receipt, err := passport.Authorize(ctx, a1.AuthorizeOptions{
    IntentName:    "trade.equity",
    ExecutorPkHex: agentPkHex,
})

// Issue a sub-delegation cert
subCert, err := passport.IssueSub(ctx, a1.IssueSubOptions{
    DelegatePkHex: subAgentPkHex,
    Capabilities:  []string{"trade.equity"},
    TTLSeconds:    3600,
})
```

---

## API Reference

### `A1Client`

```go
// Create a client
client := a1.NewClient(gatewayURL string, opts *a1.ClientOptions)

// Authorize a single intent
result, err := client.Authorize(ctx context.Context, req a1.AuthorizeRequest) (a1.AuthorizeResult, error)

// Authorize a batch of intents
results, err := client.AuthorizeBatch(ctx context.Context, req a1.AuthorizeBatchRequest) (a1.AuthorizeBatchResult, error)

// Issue a delegation cert (requires admin secret)
cert, err := client.IssueCert(ctx context.Context, req a1.IssueCertRequest) (a1.IssueCertResult, error)

// Revoke a cert (requires admin secret)
err := client.RevokeCert(ctx context.Context, fingerprint string) error

// Check gateway health
health, err := client.Health(ctx context.Context) (a1.HealthResult, error)
```

### `ClientOptions`

```go
type ClientOptions struct {
    Timeout     time.Duration // default: 10s
    AdminSecret string        // required for admin endpoints
}
```

### `AuthorizeRequest`

```go
type AuthorizeRequest struct {
    Chain          SignedChain
    IntentName     string
    IntentParams   map[string]any
    ExecutorPkHex  string
    IdempotencyKey string // optional
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

---

## Error Handling

```go
import a1 "github.com/dyologician/a1/sdk/go/a1/kya"

result, err := client.Authorize(ctx, req)
if err != nil {
    var authErr *a1.AuthorizationError
    if errors.As(err, &authErr) {
        // Authorization denied: expired cert, scope violation, replay, etc.
        fmt.Printf("Denied: %s (code: %s)\n", authErr.Reason, authErr.ErrorCode)
    } else {
        // Network error or unexpected gateway response
        fmt.Printf("Gateway error: %v\n", err)
    }
}
```

---

## Running the Gateway

```bash
git clone https://github.com/dyologician/a1.git
cd a1
docker compose up -d
curl http://localhost:8080/healthz
```

---

## Development

```bash
cd sdk/go
go test -v ./...
go test -race ./...
go vet ./...
```

---

## License

MIT OR Apache-2.0. See [LICENSE-MIT](https://github.com/dyologician/a1/blob/main/LICENSE-MIT) and [LICENSE-APACHE](https://github.com/dyologician/a1/blob/main/LICENSE-APACHE).

---

*Part of the [A1](https://github.com/dyologician/a1) ecosystem. Built and maintained by dyolo ([@dyologician](https://github.com/dyologician)).*