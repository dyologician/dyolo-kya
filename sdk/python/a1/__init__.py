from .client import A1Client, AsyncA1Client, A1Error, IntentSpec, IssuedCert, AuthorizeResult
from .passport import PassportClient, PassportError, PassportReceipt, a1_guard
from .middleware import protect, inject_passport, A1Context, set_context, get_context, a1_context, MiddlewareError
from .otel import A1Tracer, A1Span, noop_tracer

__all__ = [
    "A1Client", "AsyncA1Client", "A1Error", "IntentSpec", "IssuedCert", "AuthorizeResult",
    "PassportClient", "PassportError", "PassportReceipt", "a1_guard",
    "protect", "inject_passport", "A1Context", "set_context", "get_context", "a1_context", "MiddlewareError",
    "A1Tracer", "A1Span", "noop_tracer",
]
__version__ = "2.8.0"