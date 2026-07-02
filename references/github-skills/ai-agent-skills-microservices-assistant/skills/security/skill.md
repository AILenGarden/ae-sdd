# Security Skill

## Description
Implement Spring Security with OAuth2/JWT.

## Instructions
1. Add deps: spring-boot-starter-security, spring-security-oauth2.
2. SecurityConfig: @EnableWebSecurity, filters.
3. Resource server: Validate JWT.
4. Gateway: Forward auth.

## Prompt Template for Copilot/Claude
"Generate Spring Security config for microservices with OAuth2, JWT validation."

## Guardrails
- Role-based access.
- No basic auth in prod.

## Examples
- SecurityConfig.java with .oauth2ResourceServer().