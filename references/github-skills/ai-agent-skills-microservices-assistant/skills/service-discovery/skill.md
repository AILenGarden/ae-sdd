# Service Discovery Skill

## Description
Set up service registry with Eureka.

## Instructions
1. Create Eureka server project.
2. Add @EnableEurekaServer.
3. Clients: @EnableDiscoveryClient, register in yml.

## Prompt Template for Copilot/Claude
"Generate code for Spring Cloud Eureka server and client config in a microservice."

## Guardrails
- Use peer-aware for HA.
- Local setup.

## Examples
- EurekaServerApplication.java.