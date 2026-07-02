# Monitoring Skill

## Description
Set up Actuator and Micrometer for metrics.

## Instructions
1. Add dep: spring-boot-starter-actuator, micrometer-registry-prometheus.
2. Expose endpoints: /actuator/health, /metrics.
3. Integrate with Prometheus/Grafana.

## Prompt Template for Copilot/Claude
"Generate monitoring config with Actuator and Micrometer in Spring Boot."

## Guardrails
- Secure endpoints.
- Custom metrics.

## Examples
- application.yml with management.endpoints.web.exposure.include=*.