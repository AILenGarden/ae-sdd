# Services and Controllers Skill

## Description
Build services and REST controllers.

## Instructions
1. Services: Business logic.
2. Controllers: @RestController, endpoints.

## Prompt Template for Copilot/Claude
"Generate layered Spring Boot code (separate Controller, Service, Repository classes) for [feature, e.g., Order creation]. Follow strict separation: dumb controller, business logic in service, data access in repository. Use Java 21 records for DTOs, constructor injection, @Transactional on service methods."

## Guardrails
- Use DTOs.
- Validation.

## Examples
- UserController.java.