# AI Agent for Distributed Microservices Application Assistance (Java 21 + Spring Cloud)

This repository provides a modular AI agent setup to assist teams in developing a distributed microservices application using Spring Boot, Spring Cloud, Kafka, ActiveMQ, and PostgreSQL. It includes skills (reusable prompts) for AI tools like GitHub Copilot or Claude to generate code, perform reviews, and more.

## Importance for Java Ecosystem Users
In the Java ecosystem, microservices with Spring Cloud are standard for scalable, resilient apps. This agent:
- **Promotes Best Practices**: Enforces patterns like circuit breakers, tracing, and security, reducing downtime and bugs.
- **Boosts Productivity**: Modular skills for on-demand AI help in building services, gateways, etc.
- **Scales for Teams**: Shareable repo ensures alignment on industry standards like DDD, API versioning, and observability.
- **Adapts to Modern Java**: Uses JDK 21 for virtual threads, improving concurrency in distributed systems.
- **Fault-Tolerant Focus**: Covers retry, routes, audit trails—key for production-grade microservices.

This setup is inspired by agentic AI patterns, tailored for Java devs building cloud-native apps.

## Implementation Steps
1. **Clone the Repo**: `git clone <your-repo-url>`
2. **Integrate into Your Project**: Copy the `ai-agent-microservices-assistant/` folder into your main microservices Git repo (e.g., under `/docs/` or root). Or use as submodule: `git submodule add <this-repo-url> ai-agent`.
3. **Customize**: Update skills/guidelines for your domain (e.g., add domain-specific entities).
4. **Commit Changes**: Add customizations and push.

## Configuration Steps
- **Prerequisites**:
  - JDK 21 (e.g., via SDKMAN: `sdk install java 21.0.2-open`).
  - Maven 3.9+.
  - IDE: VS Code with GitHub Copilot, or IntelliJ.
  - AI Access: Copilot subscription; Claude.ai account.
  - Docker for brokers/containers.
- **Project Config**:
  - In each microservice pom.xml: `<java.version>21</java.version>`, add Spring Cloud deps.
  - IDE: Set Java compiler to 21.
  - For AI: Enable Copilot in VS Code; no extra for Claude.
- **Broker/Config Setup**: Follow skills for service-discovery, config-server, etc.

## Use Steps
1. **For Coding Help**:
   - In VS Code: `// Load [skill-name] from ai-agent: [paste prompt from skill.md] + query`.
   - In Claude: "You are the microservices agent. Load [skill-name]: [paste skill.md]. Now, [query]."
2. **For Code Reviews**:
   - Use `templates/code-review-template.md`: Paste into AI with code.
3. **Team Sharing**:
   - Share repo link.
   - Instruct: "Reference agent.md; load skills on demand."
4. **Example Workflow**:
   - Bootstrap: Load `project-setup` for a microservice.
   - Infrastructure: Load `api-gateway`, `security`.
   - Build: Load `entities-repos`, `services-controllers`.
   - Resilience: Load `resilience` for retries.
   - Migrate: Load `migration` for upgrading from legacy systems.
   - Deploy: Load `deployment` for Docker.

For questions, open issues.