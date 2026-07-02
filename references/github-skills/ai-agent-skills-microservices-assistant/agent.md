# AI Agent for Distributed Microservices Application Assistance

## Agent Role
This agent assists team members with coding, code reviews, and development in our Java 21 + Spring Boot + Spring Cloud distributed microservices project. It focuses on building resilient, scalable services with API Gateway, security, retry, routes, audit trails, and more.

- **Core Principles**: Provide modular, reusable help. Load skills on demand. Assume good intent; be truthful; no moralizing.
- **Tech Stack**: Java 21, Spring Boot 3+, Spring Cloud (Gateway, Config, Eureka, Resilience4j, Feign), Kafka/ActiveMQ, JPA (PostgreSQL).
- **Usage**: Query AI with "As the microservices agent, load [skill-name] skill and [task]."

## Skill Index (Load on Demand)
- **project-setup**: Bootstrap a microservice with dependencies.
- **service-discovery**: Set up Eureka/Consul for registry.
- **config-server**: Centralized config management.
- **api-gateway**: API Gateway with routes.
- **security**: Spring Security/OAuth2.
- **resilience**: Circuit breaker, retry limits.
- **distributed-tracing**: Zipkin/Micrometer tracing.
- **audit-trail**: Logging/interceptors for audits.
- **inter-service-comm**: Feign clients for calls.
- **messaging**: Kafka/ActiveMQ integration.
- **monitoring**: Actuator/Micrometer metrics.
- **entities-repos**: JPA entities/repositories.
- **services-controllers**: Services and controllers.
- **testing**: Tests and run instructions.
- **deployment**: Docker/K8s basics.
- **migration**: Guide migrations from older Spring/legacy frameworks to Spring Boot 3.2.x.

## How the Agent Works
1. Query: E.g., "Set up API Gateway."
2. Load skill: Append skill.md to prompt.
3. Execute: AI generates code.
4. Review: Use guidelines.md.

Version: 1.0.