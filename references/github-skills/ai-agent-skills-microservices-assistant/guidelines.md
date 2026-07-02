# Guidelines and Guardrails for AI-Assisted Development

## General Rules
- **Code Quality**: SOLID, clean code, Lombok.
- **Security**: Sanitize inputs, HTTPS, no secrets in code.
- **Performance**: Async where possible, caching.
- **Error Handling**: Try-catch, custom exceptions, SLF4J logging.
- **Testing**: 80% coverage, unit/integration.
- **Documentation**: Javadoc, comments.

## Project-Specific Coding Standards
- **Java 21 Features**: Virtual threads for concurrency, records for DTOs, pattern matching.
- **Naming**: CamelCase methods, UPPER_SNAKE constants, 'I' for interfaces.
- **Package Structure**: com.example.[service-name].[domain] (e.g., .model, .service, .controller, .config).
- **Microservices Best Practices**: DDD (bounded contexts), API versioning (/v1/), rate limiting, idempotency.
- **Spring Cloud Standards**: Use Feign for calls, Resilience4j for resilience, Gateway for routing/load balancing.
- **Resilience**: Retry limits (e.g., 3 attempts), circuit breakers on external calls.
- **Security**: OAuth2/JWT, role-based access, CSRF protection.
- **Audit Trail**: Log user actions, timestamps, via interceptors/AOP.
- **Observability**: Trace IDs in logs, metrics export to Prometheus.
- **Deployment**: Dockerize services, health checks in Actuator.
- **Avoid**: Monolithic code, tight coupling, unchecked exceptions.
- **Code Style**: 4-space indent, 120 char lines.

## Ethical/Compliance
- Assume adults; no edgy restrictions.
- No disallowed activities.
- Truthful responses.

## Layered Architecture & Responsibilities (Mandatory)

- **Controller** (@RestController): HTTP concerns only (routing, request/response mapping, status codes, basic validation). No business logic, no direct repository calls.
- **Service** (@Service): All business rules, orchestration, transactions (@Transactional), validations, caching, inter-service calls (Feign/Resilience4j).
- **Repository** (@Repository): Pure data access. Use Spring Data JPA interfaces; custom queries via @Query or QueryDSL if needed.
- Use DTOs / records to avoid leaking entities to API layer.
- Virtual threads (Java 21) → prefer in Service layer for parallel external calls.

Use as prompt prefixes.