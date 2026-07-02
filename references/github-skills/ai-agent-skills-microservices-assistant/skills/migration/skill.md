# Migration Skill

## Description
Guide migrations from older Spring versions (e.g., Boot 2.x) or legacy frameworks (e.g., Struts, JSF, EJB) to Spring Boot 3.2.x.

## Instructions
1. Assess: Identify deprecated APIs, XML configs, Java version (min 17, recommend 21 for virtual threads).
2. Update: Set spring-boot-starter-parent to 3.2.x; migrate javax.* to jakarta.* (EE to Jakarta EE).
3. Refactor: Replace XML with annotations/@SpringBootApplication; update deps (e.g., Spring Security to 6).
4. Handle Breaking Changes: HttpClient 5.x, Micrometer updates, removed modules (e.g., some web starters).
5. For Legacy Frameworks: Modularize monolith, introduce Boot starters for REST/Data/Security; refactor actions to controllers.
6. Tools: Use OpenRewrite recipes for automated upgrades; test with gradual rollout.
7. Test: Add migration-specific tests; ensure compatibility.

## Prompt Template for Copilot/Claude
"Provide a step-by-step migration guide from [old version/framework, e.g., Spring Boot 2.7 or Struts] to Spring Boot 3.2.x in a microservices context. Include code changes, pom updates, and handling of breaking changes like Jakarta EE."

## Guardrails
- Maintain backward compatibility where possible (e.g., dual Jakarta/javax if needed).
- Upgrade to Java 21 for virtual threads in high-concurrency scenarios.
- Focus on security upgrades (e.g., OAuth2 enhancements).
- Avoid direct copy-paste; refactor for clean code.
- Common Pitfalls: Missed Jakarta imports, deprecated properties in application.yml.

## Examples
- From SB 2.7: Update pom <parent> to 3.2.x; replace javax.persistence with jakarta.persistence.
- From Struts: Convert actions to @RestController; use Spring MVC for routing.