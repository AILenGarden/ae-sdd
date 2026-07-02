package ai.nubase.common.config;

import ai.nubase.common.multitenancy.DatabaseMultiTenantConnectionProvider;
import ai.nubase.common.multitenancy.DatabaseTenantResolver;
import lombok.extern.slf4j.Slf4j;
import org.hibernate.cfg.AvailableSettings;
import org.springframework.boot.autoconfigure.orm.jpa.HibernatePropertiesCustomizer;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

import java.util.Map;

/**
 * Hibernate configuration for database-level isolation.
 * <p>
 * Differences from the auth module's HibernateConfig:
 * - SchemaMultiTenantConnectionProvider -> DatabaseMultiTenantConnectionProvider
 * - SchemaResolver -> DatabaseTenantResolver
 * - Does not use SET search_path; instead routes to different databases via RoutingDataSource
 * <p>
 * This configuration is only enabled for database-level isolation.
 *
 * @author nubase
 * @since 2025-01-02
 */
@Slf4j
@Configuration
public class DatabaseHibernateConfig {

    /**
     * Configures Hibernate to use the database-level multi-tenancy strategy.
     */
    @Bean
    public HibernatePropertiesCustomizer databaseHibernatePropertiesCustomizer(
            DatabaseMultiTenantConnectionProvider connectionProvider,
            DatabaseTenantResolver tenantResolver) {

        log.info("Configuring Hibernate for database-level multi-tenancy");

        return new HibernatePropertiesCustomizer() {
            @Override
            public void customize(Map<String, Object> hibernateProperties) {
                // Set the multi-tenant connection provider (uses RoutingDataSource)
                hibernateProperties.put(
                        AvailableSettings.MULTI_TENANT_CONNECTION_PROVIDER,
                        connectionProvider
                );

                // Set the tenant identifier resolver (reads database_key from DatabaseContext)
                hibernateProperties.put(
                        AvailableSettings.MULTI_TENANT_IDENTIFIER_RESOLVER,
                        tenantResolver
                );

                log.debug("Hibernate multi-tenancy configured:");
                log.debug("  - Connection Provider: DatabaseMultiTenantConnectionProvider");
                log.debug("  - Tenant Resolver: DatabaseTenantResolver");
                log.debug("  - Strategy: Database-level isolation");
            }
        };
    }
}
