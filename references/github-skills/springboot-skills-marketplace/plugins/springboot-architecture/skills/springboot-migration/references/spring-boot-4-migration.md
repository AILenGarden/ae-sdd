# Spring Boot 4.0 Migration Guide

> **Official Documentation**: [Spring Boot 4.0 Migration Guide](https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Migration-Guide)

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Migration Strategy](#migration-strategy)
3. [Dependency Changes](#dependency-changes)
4. [Code Changes](#code-changes)
5. [Configuration Changes](#configuration-changes)
6. [Test Changes](#test-changes)
7. [Verification](#verification)

---

## System Requirements

### Required Versions

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **Java** | 17 | 21+ (25 for latest features) |
| **Spring Boot** | 4.0.0 | 4.0.x (latest stable) |
| **Jakarta EE** | 11 | 11 |
| **Servlet** | 6.1 | 6.1 |

**Java Version Note:**
- Minimum: Java 17
- Recommended: Java 21 (LTS) or Java 25 (latest features)

---

## Migration Strategy

### Recommended Approach: Gradual Migration with Classic Starters

Spring Boot 4.0 introduces **modular architecture** with granular starters. For existing projects, Spring Boot provides "classic" starters to ease migration:

```xml
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-classic</artifactId>
</dependency>
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-test-classic</artifactId>
    <scope>test</scope>
</dependency>
```

**Benefits:**
- Fewer breaking changes upfront
- Easier rollback if issues arise
- Gradual migration to modular starters

**Recommended Migration Path:**
1. Upgrade to classic starters first
2. Verify application works
3. Incrementally migrate to modular starters (optional)

### Alternative: Direct Migration

Update directly to new modular starters (web→webmvc, aop→aspectj, etc.)

**Benefits:**
- Clean, modern codebase from the start
- No technical debt from classic starters

**Drawbacks:**
- More changes at once
- Higher risk

---

## Dependency Changes

### 1. Web Starter Rename

**Change:**

| Old | New |
|-----|-----|
| `spring-boot-starter-web` | `spring-boot-starter-webmvc` |

**Migration:**

```xml
<!-- Before -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-web</artifactId>
</dependency>

<!-- After (Option A: Direct) -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-webmvc</artifactId>
</dependency>

<!-- After (Option B: Classic) -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-classic</artifactId>
</dependency>
```

### 2. AOP Starter Rename

**Change:**

| Old | New |
|-----|-----|
| `spring-boot-starter-aop` | `spring-boot-starter-aspectj` |

**IMPORTANT:** Before migrating, **review if you actually need this starter**.

**When you need it:**
- Your application uses AspectJ annotations (typically in `org.aspectj.lang.annotation` package)
- Using `@Retryable` / `@ConcurrencyLimit` annotations
- Using certain Micrometer annotations like `@Timed`
- Spring Modulith event handling (if using aspect-based features)

**When you DON'T need it:**
- Only using Spring AOP features like `@Async`, `@Transactional`, `@Cacheable` (these work without the starter)
- Spring context automatically includes `spring-aop` module

**Migration (if needed):**

```xml
<!-- Before -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-aop</artifactId>
</dependency>

<!-- After -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-aspectj</artifactId>
</dependency>
```

### 3. RestClient / WebClient Modular Starters (NEW)

**Change:** Boot 4 splits HTTP client support out of the web starter. `spring-boot-starter-webmvc` no longer includes RestClient auto-configuration.

| Client | New Starter |
|--------|-------------|
| `RestClient` / `RestTemplate` | `spring-boot-starter-restclient` |
| `WebClient` | `spring-boot-starter-webclient` |

**Migration:**

```xml
<!-- Add if using RestClient or RestTemplate -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-restclient</artifactId>
</dependency>

<!-- Add if using WebClient -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-webclient</artifactId>
</dependency>
```

**Testing starters:**
- `spring-boot-starter-restclient-test` — for RestClient testing support
- `spring-boot-starter-webclient-test` — for WebClient testing support

Source: [Spring Boot 4.0 Migration Guide](https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Migration-Guide)

### 4. Flyway Migration

**Change:** Flyway now requires explicit starter

**Migration:**

```xml
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-flyway</artifactId>
</dependency>
<dependency>
    <groupId>org.flywaydb</groupId>
    <artifactId>flyway-database-postgresql</artifactId>
</dependency>
```

### 4. Security Test Starter

**Change:**

| Old | New |
|-----|-----|
| `spring-security-test` | `spring-boot-starter-security-test` |

**Migration:**

```xml
<!-- Before -->
<dependency>
    <groupId>org.springframework.security</groupId>
    <artifactId>spring-security-test</artifactId>
    <scope>test</scope>
</dependency>

<!-- After -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-security-test</artifactId>
    <scope>test</scope>
</dependency>
```

### 5. Retry/Resilience Annotations

**Sample usage (from `sivaprasadreddy/spring-boot-4-features`):**
- Package: `org.springframework.resilience.annotation.*`
- Annotations: `@Retryable`, `@ConcurrencyLimit`
- Enablement: `@EnableResilientMethods`
- Parameters used: `includes`, `maxAttempts`, `delay`, `multiplier`
- Circuit breaker is still external (Resilience4j)

Spring Retry is now maintenance-only, superseded by native resilience in Spring Framework 7.
Migrate to `org.springframework.resilience.annotation.*` and remove `spring-retry` dependency.

**Required dependencies:**

```xml
<!-- AOP support required for @Retryable / @ConcurrencyLimit -->
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-aspectj</artifactId>
</dependency>
```

**Migration steps:**

1. Remove `spring-retry` dependency from pom.xml
2. Replace `@EnableRetry` with `@EnableResilientMethods`
3. Change imports: `org.springframework.retry.annotation.*` → `org.springframework.resilience.annotation.*`
4. Update annotation parameters: `retryFor` → `includes`, `backoff = @Backoff(delay = 1000)` → `delay = 1000`
5. Remove `@Recover` methods (use try-catch or programmatic `RetryTemplate` instead)

**Code changes:**

```java
// Config
import org.springframework.resilience.annotation.EnableResilientMethods;

@Configuration
@EnableResilientMethods
public class ResilienceConfig {}

// Service
import org.springframework.resilience.annotation.Retryable;

@Retryable(
    includes = {SomeException.class},
    maxAttempts = 4,
    delay = 1000,
    multiplier = 2
)
public void methodWithRetry() {
    // ...
}
```

Reference: https://github.com/spring-projects/spring-retry (maintenance-only status)

### 6. Web MVC Test Starter

**New modular test starter:**

```xml
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-webmvc-test</artifactId>
    <scope>test</scope>
</dependency>
```

**Note:** Only needed if using modular architecture. Classic test starter includes this.

---

## Code Changes

### 1. Jackson 3 Migration (CRITICAL)

**Breaking Change:** Jackson upgraded from 2.x to 3.x

#### Group ID Changes

```xml
<!-- Old (Jackson 2.x) -->
<dependency>
    <groupId>com.fasterxml.jackson.core</groupId>
    <artifactId>jackson-databind</artifactId>
</dependency>

<!-- New (Jackson 3.x) -->
<dependency>
    <groupId>tools.jackson.core</groupId>
    <artifactId>jackson-databind</artifactId>
</dependency>

<!-- Exception: jackson-annotations stays with old group -->
<dependency>
    <groupId>com.fasterxml.jackson.core</groupId>
    <artifactId>jackson-annotations</artifactId>
</dependency>
```

#### Class Renames

| Old (Jackson 2) | New (Jackson 3) |
|-----------------|-----------------|
| `Jackson2ObjectMapperBuilderCustomizer` | `JsonMapperBuilderCustomizer` |
| `@JsonComponent` | `@JacksonComponent` |
| `JsonObjectSerializer` | `ObjectValueSerializer` |
| `JsonValueDeserializer` | `ObjectValueDeserializer` |

#### Import Changes

```java
// Old
import com.fasterxml.jackson.databind.ObjectMapper;

// New
import tools.jackson.databind.json.JsonMapper;

// Usage
JsonMapper mapper = JsonMapper.builder().build();
```

#### Compatibility Module (For Gradual Migration)

```xml
<dependency>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-jackson2</artifactId>
</dependency>
```

### 2. Package Relocations

#### Bootstrap Registry

```java
// Old
import org.springframework.boot.BootstrapRegistry;
import org.springframework.boot.BootstrapContext;

// New
import org.springframework.boot.bootstrap.BootstrapRegistry;
import org.springframework.boot.bootstrap.BootstrapContext;
```

#### Entity Scan

```java
// Old
import org.springframework.boot.autoconfigure.domain.EntityScan;

// New
import org.springframework.boot.persistence.autoconfigure.EntityScan;
```

---

## Test Changes

### 1. MockBean → MockitoBean

**Change:** Migrate from **deprecated** to **standard** annotations

| Deprecated (still available in 4.0) | Standard (since 3.4) |
|-----|-----|
| `@MockBean` | `@MockitoBean` |
| `@SpyBean` | `@MockitoSpyBean` |

**Timeline:**
- **Spring Boot 3.4** (Spring Framework 6.2): `@MockitoBean` introduced, `@MockBean` deprecated
- **Spring Boot 4.0**: `@MockBean` **still available but deprecated** (will be removed in future version)
- **Future Spring Boot version**: `@MockBean` will be removed completely

**Migration urgency:** While `@MockBean` still works in Spring Boot 4.0, migrate to `@MockitoBean` now to avoid breaking changes in future releases.

**CRITICAL CONSTRAINT:** `@MockitoBean` and `@MockitoSpyBean` can **ONLY** be used on test class fields, **NOT** in `@Configuration` or `@TestConfiguration` classes.

**Migration - Test Class Fields (Standard Case):**

```java
// Before (Using deprecated annotations - still works in 4.0)
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.boot.test.mock.mockito.SpyBean;

@SpringBootTest
class UserServiceTest {
    @MockBean  // Deprecated since 3.4, still available in 4.0
    private UserRepository userRepository;

    @SpyBean  // Deprecated since 3.4, still available in 4.0
    private EmailService emailService;
}

// After (Recommended - using standard annotations)
import org.springframework.boot.test.mock.mockito.MockitoBean;
import org.springframework.boot.test.mock.mockito.MockitoSpyBean;

@SpringBootTest
class UserServiceTest {
    @MockitoBean  // Standard since 3.4, recommended for 4.0+
    private UserRepository userRepository;

    @MockitoSpyBean  // Standard since 3.4, recommended for 4.0+
    private EmailService emailService;
}
```

**Migration - @TestConfiguration Classes (Special Handling Required):**

If you were using `@MockBean` in `@TestConfiguration` classes, you **MUST** move them to test class fields:

```java
// Before (Spring Boot 3.x) - @MockBean in @TestConfiguration
@TestConfiguration
class TestConfig {
    @MockBean
    private UserRepository userRepository;
}

@SpringBootTest
@Import(TestConfig.class)
class UserServiceTest {
    // Test uses mocked repository from config
}

// After (Spring Boot 4.0) - Move to test class field
@SpringBootTest
class UserServiceTest {
    @MockitoBean  // Must be declared on test class field
    private UserRepository userRepository;
}
```

**Alternative for Complex Test Configurations:**

If you need to share mocks across multiple test classes, create a custom annotation:

```java
// Custom annotation combining multiple mocks
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
@MockitoBean(UserRepository.class)
@MockitoBean(OrderRepository.class)
@MockitoSpyBean(EmailService.class)
public @interface CommonTestMocks {
}

// Use in test classes
@SpringBootTest
@CommonTestMocks
class UserServiceTest {
    @Autowired
    private UserRepository userRepository;  // Will be mocked
}
```

**Temporary Workaround (Spring Boot 3.4+ including 4.0):**

If you're not ready to migrate yet, suppress deprecation warnings:

```java
// Works in Spring Boot 3.4+ and 4.0 (deprecated but still available)
// Will be removed in a future Spring Boot version
@SpringBootTest
@SuppressWarnings("removal")
class UserServiceTest {
    @MockBean  // Deprecated since 3.4, still works in 4.0
    private UserRepository userRepository;
}
```

**Important:**
- `@MockBean` **still works** in Spring Boot 4.0, but is deprecated
- It **will be removed** in a future Spring Boot version
- Migrate to `@MockitoBean` soon to avoid future breaking changes
- Per [official Spring Boot 4.0 Migration Guide](https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Migration-Guide): "@MockBean and @SpyBean support will be removed in the future"

### 2. WebMvcTest Package Relocation

```java
// Old
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;

// New
import org.springframework.boot.webmvc.test.autoconfigure.WebMvcTest;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
```

### 3. Auto-Configuration in @SpringBootTest

**Change:** MockMvc no longer auto-configured in `@SpringBootTest`

```java
// Before (Spring Boot 3)
@SpringBootTest(webEnvironment = WebEnvironment.RANDOM_PORT)
class MyControllerTest {
    @Autowired
    private MockMvc mockMvc; // Auto-configured
}

// After (Spring Boot 4)
@SpringBootTest(webEnvironment = WebEnvironment.RANDOM_PORT)
@AutoConfigureMockMvc  // Now required
class MyControllerTest {
    @Autowired
    private MockMvc mockMvc;
}
```

### 4. TestRestTemplate → RestTestClient

**Change:** `TestRestTemplate` is deprecated. Use `RestTestClient` from Spring Framework 7.

| Old | New |
|-----|-----|
| `TestRestTemplate` | `RestTestClient` |
| `@AutoConfigureWebTestClient` | `@AutoConfigureRestTestClient` |

**Migration:**

```java
// Before (Spring Boot 3)
import org.springframework.boot.test.web.client.TestRestTemplate;

@SpringBootTest(webEnvironment = WebEnvironment.RANDOM_PORT)
class UserControllerTest {
    @Autowired
    private TestRestTemplate restTemplate;

    @Test
    void shouldGetUser() {
        ResponseEntity<User> response = restTemplate.getForEntity("/users/1", User.class);
        assertEquals(HttpStatus.OK, response.getStatusCode());
    }
}

// After (Spring Boot 4)
import org.springframework.test.web.servlet.client.RestTestClient;

@SpringBootTest(webEnvironment = WebEnvironment.RANDOM_PORT)
@AutoConfigureRestTestClient
class UserControllerTest {
    @Autowired
    private RestTestClient client;

    @Test
    void shouldGetUser() {
        client.get()
                .uri("/users/1")
                .exchange()
                .expectStatus().isOk()
                .expectBody(User.class);
    }
}
```

Source: [RestTestClient :: Spring Framework](https://docs.spring.io/spring-framework/reference/testing/resttestclient.html)

### 5. HTTP Service Client: Manual Setup → @ImportHttpServices

**Change:** Manual `HttpServiceProxyFactory` bean wiring is replaced by `@ImportHttpServices` auto-configuration (Spring Framework 7.0).

Note: `@HttpExchange` itself exists since Framework 6.0. Only `@ImportHttpServices` is new.

```java
// Before (Spring Boot 3 / Framework 6)
@Bean
ProductClient productClient(RestClient.Builder builder) {
    RestClient restClient = builder.baseUrl("http://api.example.com").build();
    return HttpServiceProxyFactory
            .builderFor(RestClientAdapter.create(restClient))
            .build()
            .createClient(ProductClient.class);
}

// After (Spring Boot 4 / Framework 7)
@Configuration
@ImportHttpServices(group = "product", types = ProductClient.class)
public class ClientConfig { }
```

```yaml
spring:
  http:
    clients:
      connect-timeout: 5s
    serviceclient:
      product:  # Group name (matches @ImportHttpServices group)
        base-url: http://api.example.com
        read-timeout: 10s
```

Source: [Spring Boot HTTP Service Client](https://docs.spring.io/spring-boot/reference/io/rest-client.html)

### 6. Native API Versioning (New Feature)

**What's new:** Spring Framework 7.0 adds native API versioning via `version` attribute on `@GetMapping`, `@PostMapping`, etc.

**No `@ApiVersion` annotation exists** — version is declared directly on mapping annotations.

```java
@GetMapping(value = "/search", version = "1.0")
public List<ProductVM> searchV1(@RequestParam("q") String query) { ... }

@GetMapping(value = "/search", version = "2.0")
public List<ProductEnrichedVM> searchV2(@RequestParam("q") String query) { ... }
```

```yaml
spring:
  mvc:
    apiversion:
      enabled: true
      strategy: header
      default-version: "1.0"
      header-name: "API-Version"
```

**Migration:** Replace custom versioning interceptors/filters with native configuration above.

Source: [API Versioning in Spring](https://spring.io/blog/2025/09/16/api-versioning-in-spring/)

### 7. @ConcurrencyLimit (New Feature)

**What's new:** Native concurrency limiting via `org.springframework.resilience.annotation.ConcurrencyLimit` (Spring Framework 7.0). Requires `@EnableResilientMethods`.

```java
@Configuration
@EnableResilientMethods
public class ResilienceConfig { }

@Service
public class ReportService {
    @ConcurrencyLimit(2)
    public void processExpensiveOperation(String id) { ... }
}
```

Source: [Core Spring Resilience Features](https://spring.io/blog/2025/09/09/core-spring-resilience-features/)

---

## Configuration Changes

### 1. Jackson Properties

```properties
# Old
spring.jackson.read.allow-trailing-comma=true
spring.jackson.write.indent-output=true

# New
spring.jackson.json.read.allow-trailing-comma=true
spring.jackson.json.write.indent-output=true
```

### 2. Health Probes (New Default Behavior)

**Change:** Liveness/readiness probes **enabled by default** in Spring Boot 4

```properties
# To disable (if not needed):
management.endpoint.health.probes.enabled=false
```

**Endpoints available:**
- `/actuator/health/liveness`
- `/actuator/health/readiness`

### 3. DevTools Live Reload (New Default Behavior)

**Change:** Live reload **disabled by default** in Spring Boot 4

```properties
# To enable:
spring.devtools.livereload.enabled=true
```

---

## Verification

### Build Verification

```bash
# Clean build
./mvnw clean package

# Check for deprecation warnings
./mvnw clean compile | grep -i deprecated
```

### Test Verification

```bash
# Run all tests
./mvnw clean verify

# Run specific test categories
./mvnw test -Dtest=*Test
./mvnw test -Dtest=*IT
```

### Runtime Verification

1. **Application Startup**
   - Check logs for errors
   - Look for deprecation warnings
   - Verify bean creation

2. **Critical Endpoints**
   - Test main API endpoints
   - Verify authentication/authorization
   - Check database connectivity

3. **Health Checks**
   - `GET /actuator/health`
   - `GET /actuator/health/liveness`
   - `GET /actuator/health/readiness`

### Common Issues After Migration

#### Issue 1: NoSuchMethodError or NoClassDefFoundError

**Cause:** Conflicting Jackson versions

**Solution:**
```bash
# Check dependency tree
./mvnw dependency:tree | grep jackson

# Resolve conflicts by excluding old Jackson from transitive dependencies
```

#### Issue 2: Tests Fail with LazyInitializationException

**Cause:** Changes in transaction management

**Solution:** Use `@Transactional` on test methods or fetch associations eagerly

#### Issue 3: @Retryable Not Working

**Cause:** Missing `@EnableResilientMethods` or missing AOP support

**Solution:**
1. Add `spring-boot-starter-aspectj`
2. Add `@EnableResilientMethods` to a `@Configuration` class
3. Verify imports use `org.springframework.resilience.annotation.*` (not legacy `org.springframework.retry.*`)

---

## Migration Checklist

### Phase 1: Dependencies
- [ ] Update Spring Boot version to 4.0.x
- [ ] Rename `spring-boot-starter-web` to `-webmvc` (or use classic)
- [ ] Rename `spring-boot-starter-aop` to `-aspectj`
- [ ] Add `spring-boot-starter-restclient` if using RestClient/RestTemplate
- [ ] Add `spring-boot-starter-webclient` if using WebClient
- [ ] Update `spring-security-test` to Spring Boot starter
- [ ] Add Spring Retry with explicit version (if using Spring Retry directly)
- [ ] Add Flyway starter (if using database migrations)
- [ ] Update Testcontainers dependencies (if used)

### Phase 2: Code
- [ ] Update Jackson imports (if using custom ObjectMapper)
- [ ] **Recommended:** Update `@MockBean` to `@MockitoBean` (deprecated but still works)
- [ ] **Recommended:** Update `@SpyBean` to `@MockitoSpyBean` (deprecated but still works)
- [ ] Update `@WebMvcTest` import
- [ ] Add `@AutoConfigureMockMvc` where needed
- [ ] Fix retry/resilience imports and annotations
- [ ] Update package relocations (EntityScan, BootstrapRegistry)
- [ ] Replace `TestRestTemplate` with `RestTestClient`
- [ ] Replace manual `HttpServiceProxyFactory` setup with `@ImportHttpServices`
- [ ] Replace custom API versioning with native `spring.mvc.apiversion.*`
- [ ] Add `@EnableResilientMethods` for `@ConcurrencyLimit` / native `@Retryable`

### Phase 3: Configuration
- [ ] Update Jackson properties (if used)
- [ ] Configure health probes (if needed)
- [ ] Configure DevTools live reload (if needed)

### Phase 4: Testing
- [ ] Run all unit tests
- [ ] Run all integration tests
- [ ] Test application startup
- [ ] Verify critical endpoints
- [ ] Check health actuator endpoints
- [ ] Performance testing (if applicable)

---

## References

- [Spring Boot 4.0 Migration Guide (Official)](https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Migration-Guide)
- [Spring Boot 4.0 Release Notes](https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Release-Notes)
- [Spring Framework 7.0 What's New](https://docs.spring.io/spring-framework/reference/7.0/whatsnew.html)
- [Jackson 3.0 Migration Guide](https://github.com/FasterXML/jackson/wiki/Jackson-Release-3.0)
- [Vlad Mihalcea - Spring Transaction Best Practices](https://vladmihalcea.com/spring-transaction-best-practices/)
- [RestTestClient :: Spring Framework](https://docs.spring.io/spring-framework/reference/testing/resttestclient.html)
- [HTTP Service Client Enhancements (Blog)](https://spring.io/blog/2025/09/23/http-service-client-enhancements/)
- [API Versioning in Spring (Blog)](https://spring.io/blog/2025/09/16/api-versioning-in-spring/)
- [Core Spring Resilience Features (Blog)](https://spring.io/blog/2025/09/09/core-spring-resilience-features/)
