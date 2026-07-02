package ai.nubase.common.multitenancy;

import ai.nubase.common.context.MultiTenancyContext;
import lombok.extern.slf4j.Slf4j;
import org.hibernate.context.spi.CurrentTenantIdentifierResolver;
import org.springframework.stereotype.Component;

/**
 * Database-level tenant identifier resolver.
 * <p>
 * <p>
 * This class is only enabled with database-level isolation.
 *
 * @author nubase
 * @since 2025-01-02
 */
@Slf4j
@Component
public class DatabaseTenantResolver implements CurrentTenantIdentifierResolver {

    /**
     * Resolves the current tenant identifier.
     * <p>
     * In database mode the tenant identifier is the database_key, not schema_name.
     *
     * @return the key of the current database, or "default" if not set
     */
    @Override
    public String resolveCurrentTenantIdentifier() {
        // Read the current database key from MultiTenancyContext
        String databaseKey = MultiTenancyContext.getDatabaseKey();

        if (databaseKey == null || databaseKey.isBlank()) {
            // It is normal to have no context during Hibernate initialization at application
            // startup. Returning "default" lets Hibernate use RoutingDataSource's default data
            // source.
            log.debug("MultiTenancyContext database key is not set, using 'default' as tenant identifier. " +
                    "This is normal during application startup.");
            return "default";
        }

        log.trace("Resolved tenant identifier: {}", databaseKey);
        return databaseKey;
    }

    /**
     * Whether to validate existing sessions.
     * <p>
     * Returning true makes Hibernate verify that the tenant identifier of an existing session
     * matches the current request. If not, the old session is closed and a new one is created.
     *
     * @return true
     */
    @Override
    public boolean validateExistingCurrentSessions() {
        return true;
    }
}
