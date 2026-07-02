
# Project Setup Skill

## Description
Bootstrap a Spring Boot microservice with Spring Cloud dependencies.

## Instructions
1. Use Initializr/Maven.
2. Add deps: Web, JPA, Cloud Bootstrap, Eureka Client, Config Client, Gateway (if applicable), Resilience4j, etc.
3. Config application.properties/yml.

## Prompt Template for Copilot/Claude
"Generate pom.xml and application.yml for a Spring Boot 3 microservice using Java 21, with Spring Cloud deps for discovery, config, resilience."

## Guardrails
- Java 21.
- Modular for individual services.

## Examples
- pom.xml with <spring-cloud.version>2023.0.0</spring-cloud.version>.