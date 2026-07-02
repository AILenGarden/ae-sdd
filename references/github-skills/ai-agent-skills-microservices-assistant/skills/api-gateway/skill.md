# API Gateway Skill

## Description
Set up Spring Cloud Gateway for routes, load balancing.

## Instructions
1. Create gateway project.
2. @EnableDiscoveryClient.
3. application.yml: routes with predicates/filters.
4. Add rate limiting, retries.

## Prompt Template for Copilot/Claude
"Generate Spring Cloud Gateway config with routes, predicates, filters for microservices."

## Guardrails
- Dynamic routing via discovery.
- Secure routes.

## Examples
- application.yml with spring.cloud.gateway.routes.