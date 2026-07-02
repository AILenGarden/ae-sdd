# Inter-Service Communication Skill

## Description
Set up Feign clients for synchronous calls.

## Instructions
1. Add dep: spring-cloud-starter-openfeign.
2. @EnableFeignClients.
3. Interface: @FeignClient(name="service").

## Prompt Template for Copilot/Claude
"Generate Feign client for inter-microservice calls in Spring Cloud."

## Guardrails
- Error decoding.
- Load balancing.

## Examples
- UserClient.java with @GetMapping.