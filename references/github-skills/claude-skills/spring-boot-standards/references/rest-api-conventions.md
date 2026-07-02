# REST API Conventions — Full Standard

The complete URL, status-code, versioning, pagination, idempotency, and error-contract
standard. SKILL.md carries the essentials; this file is the reference for design reviews.

## URL grammar

```
/api/v{major}/{collection}[/{id}[/{sub-collection}[/{sub-id}]]]
```

| Rule | ✅ Good | ❌ Bad |
|---|---|---|
| Plural nouns for collections | `/api/v1/orders` | `/api/v1/order`, `/api/v1/orderList` |
| HTTP method carries the verb | `POST /api/v1/orders` | `/api/v1/createOrder` |
| kebab-case for multi-word segments | `/api/v1/payment-methods` | `/paymentMethods`, `/payment_methods` |
| IDs are opaque path segments | `/orders/7f3a…` | `/orders?id=7f3a…` for single fetch |
| Max two nesting levels | `/orders/{id}/lines` | `/customers/{id}/orders/{id}/lines/{id}/taxes` |
| Filters as query params | `/orders?status=OPEN&customerId=…` | `/orders/open`, `/openOrders` |
| Actions that don't map to CRUD: noun-ified sub-resource | `POST /orders/{id}/cancellation` | `POST /orders/{id}/cancel` (acceptable fallback) |

Deeper relationships: flatten and filter — `/api/v1/order-lines?orderId=…` instead of a
third nesting level.

## HTTP methods

| Method | Semantics | Idempotent | Typical success |
|---|---|---|---|
| GET | Read, no side effects | Yes | 200 |
| POST | Create / non-idempotent action | No (unless Idempotency-Key) | 201 + `Location` |
| PUT | Full replace of a resource | Yes | 200 (or 204) |
| PATCH | Partial update | No by spec; design it to be | 200 |
| DELETE | Remove | Yes | 204 |

PUT with a client-generated ID may create (return 201) or replace (200). If the server owns
ID generation, creation is POST only.

## Status-code matrix

| Code | Use for | Notes |
|---|---|---|
| 200 | Successful read, update, action | Body required |
| 201 | Resource created | `Location: /api/v1/orders/{id}` header required |
| 202 | Accepted for async processing | Body should include a status URL |
| 204 | Successful delete, or update with no body | Never include a body |
| 400 | Malformed request, failed bean validation | ProblemDetail with `errors` array |
| 401 | Missing/invalid credentials | From security filter, not controllers |
| 403 | Authenticated but not allowed | Don't leak existence — consider 404 for private resources |
| 404 | Resource doesn't exist | Also for soft-deleted resources |
| 409 | State conflict | Duplicate unique key, optimistic-lock failure, illegal state transition, replayed Idempotency-Key with different payload |
| 422 | Syntactically valid, semantically unprocessable business request | e.g. cancelling an already-shipped order; some teams fold this into 409 — pick one and be consistent |
| 429 | Rate limited | Include `Retry-After` |
| 500 | Unhandled server error | Generic ProblemDetail; never expose stack traces or SQL |
| 503 | Dependency down / maintenance | Include `Retry-After` when known |

Decision rule for 400 vs 422: 400 = the request itself is wrong (shape, types, constraint
violations); 422 = the request is well-formed but the business state refuses it.

## Versioning and deprecation

- Version in the path (`/api/v1`), major versions only. Header/query versioning is out —
  path versions are cacheable, loggable, and obvious in every tool.
- Bump the major version only on breaking changes: removing/renaming a field, changing a
  type or semantics, tightening validation on existing clients.
- Non-breaking (no bump): adding optional request fields, adding response fields, adding
  endpoints. Clients must tolerate unknown response fields.
- Run old and new majors side by side during migration. Mark the old one:

```
Deprecation: true
Sunset: Sat, 28 Nov 2026 00:00:00 GMT
Link: </api/v2/orders>; rel="successor-version"
```

- Delete a major version only after traffic is zero (verify in metrics, e.g. a
  `http_server_requests` counter tagged by URI prefix).

## Pagination contract

Offset pagination via Spring's `Pageable` is the default:

```
GET /api/v1/orders?page=0&size=20&sort=createdAt,desc
```

- Default `size` 20, enforce a max (e.g. 100) — `spring.data.web.pageable.max-page-size`.
- Return a stable envelope rather than raw `Page<T>` (its JSON shape is unstable across
  Spring Data versions and warns at runtime):

```java
public record PageResponse<T>(List<T> content, int page, int size,
                              long totalElements, int totalPages) {
    public static <T> PageResponse<T> from(Page<T> p) {
        return new PageResponse<>(p.getContent(), p.getNumber(), p.getSize(),
                p.getTotalElements(), p.getTotalPages());
    }
}
```

- Always pair `sort` with a deterministic tiebreaker (`sort=createdAt,desc&sort=id,desc`)
  or rows can repeat/vanish across pages.
- For large or hot tables prefer keyset (cursor) pagination — `?after=<lastSeenId>` with an
  indexed `WHERE (created_at, id) < (?, ?)` — offset pagination degrades linearly with page
  depth. See jpa-database-patterns for the query side.

## Idempotency keys for POST

For any POST a client may retry (payments, orders, anything money- or state-adjacent):

1. Client sends `Idempotency-Key: <uuid>` (one per logical operation, reused on retry).
2. Server atomically records the key (unique constraint) with request hash + response.
3. Same key + same payload again → replay the stored response (same status, same body).
4. Same key + **different** payload → `409 Conflict`.
5. Expire keys after a documented window (e.g. 24h).

```java
@PostMapping
public ResponseEntity<OrderResponse> create(
        @RequestHeader("Idempotency-Key") String idempotencyKey,
        @Valid @RequestBody CreateOrderRequest request) { ... }
```

Missing header on an endpoint that requires it → 400 with a ProblemDetail explaining the
header. The dedup write and the business write must share one transaction (see
jpa-database-patterns), or you've just moved the duplicate.

## Error contract — RFC 9457 ProblemDetail

Every non-2xx response is `application/problem+json`:

```json
{
  "type": "https://api.example.com/problems/order-not-found",
  "title": "Order not found",
  "status": 404,
  "detail": "Order 7f3a0c2e-… does not exist",
  "instance": "/api/v1/orders/7f3a0c2e-…",
  "errors": [ { "field": "customerId", "message": "must not be null" } ]
}
```

Rules:

- `type` is a stable URI per error category — clients switch on it, not on `detail` text.
- `detail` is human-readable and safe: no stack traces, no SQL, no internal class names.
- Validation failures add an `errors` array of `{field, message}` extension members.
- Add a `traceId` extension property (from Micrometer tracing) so users can quote it in
  support tickets and you can find the log line.
- The catch-all `@ExceptionHandler(Exception.class)` logs at ERROR with the full stack and
  returns a generic 500 ProblemDetail — clients get `traceId`, not the exception message.

Complete handler implementation: `web-examples.md`.
