# Naming Conventions

## General Rules

1. **Reveal intent**: the name should answer *why* it exists, *what* it does, *how* it is used.
2. **Avoid disinformation**: do not use `accountList` if it is not a list; do not use `hp` for hypotenuse.
3. **Make meaningful distinctions**: `productInfo` vs `productData` is noise — pick one term and be consistent.
4. **Use pronounceable names**: `genymdhms` → `generationTimestamp`.
5. **Use searchable names**: a magic number `7` is not searchable; `MAX_RETRY_COUNT = 7` is.
6. **Avoid mental mapping**: a reader should not need to translate `r` to "the lowercase version of the URL with the host removed."

## Naming by Identifier Type

### Variables and Constants
- Use nouns or noun phrases.
- Booleans use `is`, `has`, `can`, `should`: `isActive`, `hasPermission`, `canEdit`.
- Constants use UPPER_SNAKE_CASE: `MAX_CONNECTIONS`, `DEFAULT_TIMEOUT_MS`.
- Collections indicate plurality: `users`, `orderItems`, `errorMessages`.

### Methods and Functions
- Use verbs or verb phrases: `calculateTotal`, `sendEmail`, `validateInput`.
- Getters: `getBalance`, `getName`, or just `balance`, `name` (language convention).
- Predicates: `isValid`, `hasAccess`, `canProceed`.
- Converters: `toJson`, `fromString`, `asArray`.
- Factory methods: `createUser`, `buildQuery`, `of`, `from`.

### Classes and Types
- Use nouns: `User`, `OrderRepository`, `PaymentGateway`.
- Avoid generic suffixes: `Manager`, `Handler`, `Processor`, `Helper`, `Utils`.
- Prefer role-based names: `ShippingCalculator` over `ShippingHelper`.
- Interfaces: avoid `I` prefix — use `Repository` not `IRepository`.

### Packages and Modules
- Use lowercase, domain-aligned names: `billing`, `authentication`, `shipping`.
- Group by feature, not by type: `orders/` not `controllers/`, `services/`, `models/`.

## Consistency Rules

| Concept | Choose One | Not |
|---------|-----------|-----|
| Retrieve data | `get` | `fetch` / `retrieve` / `find` mixed |
| Create an instance | `create` | `make` / `build` / `new` mixed |
| Remove data | `delete` | `remove` / `destroy` / `purge` mixed |
| Modify data | `update` | `edit` / `modify` / `change` mixed |

## Domain Language
Use the same terms the business uses. If the business says "policy," do not call it "rule." If the business says "claim," do not call it "request." The code should be readable by domain experts.
