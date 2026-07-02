# Resilience Skill

## Description
Add circuit breaker, retry limits with Resilience4j.

## Instructions
1. Add dep: spring-cloud-starter-circuitbreaker-resilience4j.
2. @CircuitBreaker, @Retry on methods.
3. Config: resilience4j.retry.instances.[name].maxAttempts=3.

## Prompt Template for Copilot/Claude
"Generate Resilience4j config for retry and circuit breaker in Spring Cloud microservice."

## Guardrails
- Fallback methods.
- Metrics integration.

## Examples
- Service method with @Retry(name="backend").