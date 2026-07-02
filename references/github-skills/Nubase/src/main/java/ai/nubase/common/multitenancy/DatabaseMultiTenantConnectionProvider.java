package ai.nubase.common.multitenancy;

import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.postgrest.multidb.RoutingDataSource;
import lombok.extern.slf4j.Slf4j;
import org.hibernate.engine.jdbc.connections.spi.MultiTenantConnectionProvider;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Component;

import javax.sql.DataSource;
import java.sql.Connection;
import java.sql.SQLException;

/**
 * Database-level multi-tenant connection provider.
 * <p>
 * Differences from SchemaMultiTenantConnectionProvider:
 * - Schema mode: uses a single data source and switches schemas via SET search_path.
 * - Database mode: uses RoutingDataSource to route to a different database based on
 *   MultiTenancyContext.
 * <p>
 * This class is only enabled with database-level isolation.
 *
 * @author nubase
 * @since 2025-01-02
 */
@Slf4j
@Component
public class DatabaseMultiTenantConnectionProvider implements MultiTenantConnectionProvider {

    @Autowired
    private RoutingDataSource routingDataSource;

    @Override
    public Connection getAnyConnection() throws SQLException {
        // Return any available connection (typically used when no tenant context is set).
        // Hibernate calls this method on startup, before any request context exists.
        // RoutingDataSource automatically falls back to the configured default data source
        // (the metadata database).
        log.debug("Getting any connection from RoutingDataSource (will use default DataSource if no context)");
        return routingDataSource.getConnection();
    }

    @Override
    public void releaseAnyConnection(Connection connection) throws SQLException {
        // Release the connection
        if (connection != null && !connection.isClosed()) {
            connection.close();
        }
    }

    @Override
    public Connection getConnection(String tenantIdentifier) throws SQLException {
        // In database mode, tenantIdentifier is the database_key.
        // RoutingDataSource automatically routes to the correct database based on
        // MultiTenancyContext.

        log.debug("Getting connection for database: {}", tenantIdentifier);

        // Verify that the current MultiTenancyContext is correct
        String currentDb = MultiTenancyContext.getDatabaseKey();
        if (currentDb == null) {
            log.warn("MultiTenancyContext database key is not set, but Hibernate requested connection for: {}", tenantIdentifier);
        } else if (!currentDb.equals(tenantIdentifier)) {
            log.warn("MultiTenancyContext database key mismatch: expected={}, actual={}", tenantIdentifier, currentDb);
        }

        // RoutingDataSource returns the correct connection based on
        // MultiTenancyContext.getDatabaseKey()
        Connection connection = routingDataSource.getConnection();

        log.debug("Successfully obtained connection for database: {}", tenantIdentifier);
        return connection;
    }

    @Override
    public void releaseConnection(String tenantIdentifier, Connection connection) throws SQLException {
        // In database mode there is no need to reset search_path,
        // because every database (and its connection pool) is independent.

        log.debug("Releasing connection for database: {}", tenantIdentifier);

        if (connection != null && !connection.isClosed()) {
            // Close the connection directly so it is returned to the pool
            connection.close();
        }
    }

    @Override
    public boolean supportsAggressiveRelease() {
        // Aggressive release is not supported,
        // because connections should only be released at the end of a transaction.
        return false;
    }

    @Override
    public boolean isUnwrappableAs(Class unwrapType) {
        // Supports unwrapping to the DataSource type
        return MultiTenantConnectionProvider.class.isAssignableFrom(unwrapType) ||
                DataSource.class.isAssignableFrom(unwrapType);
    }

    @Override
    @SuppressWarnings("unchecked")
    public <T> T unwrap(Class<T> unwrapType) {
        if (MultiTenantConnectionProvider.class.isAssignableFrom(unwrapType)) {
            return (T) this;
        } else if (DataSource.class.isAssignableFrom(unwrapType)) {
            return (T) routingDataSource;
        } else {
            throw new IllegalArgumentException("Cannot unwrap to type: " + unwrapType);
        }
    }
}
