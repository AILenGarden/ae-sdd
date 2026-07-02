# Distributed Tracing Skill

## Description
Set up tracing with Micrometer + Zipkin.

## Instructions
1. Add deps: spring-cloud-starter-sleuth, micrometer-tracing-bridge-brave, zipkin-reporter.
2. Run Zipkin Docker.
3. Config: sampling probability.

## Prompt Template for Copilot/Claude
"Generate code for distributed tracing with Zipkin in Spring Cloud."

## Guardrails
- 100% sampling for dev.
- Integrate with logs.

## Examples
- application.yml with spring.zipkin.baseUrl.