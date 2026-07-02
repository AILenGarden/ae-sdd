package ai.nubase.postgrest.multidb;

import ai.nubase.common.context.MultiTenancyContext;
import com.zaxxer.hikari.HikariDataSource;
import lombok.extern.slf4j.Slf4j;
import ai.nubase.postgrest.config.PostgRESTConfig;
import ai.nubase.postgrest.schema.SchemaCache;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Manages multiple SchemaCache instances - one per database
 * Provides lazy-loading and caching of schema caches
 */
@Slf4j
@Service
public class SchemaCacheManager {

    private final Map<String, SchemaCache> caches = new ConcurrentHashMap<>();
    private final RoutingDataSource routingDataSource;

    public SchemaCacheManager(RoutingDataSource routingDataSource) {
        this.routingDataSource = routingDataSource;
    }

    /**
     * Get SchemaCache for a database
     * Creates and caches if doesn't exist
     *
     * @param dbKey the database key
     * @return SchemaCache instance
     */
    public SchemaCache getSchemaCache(String dbKey) {
        return caches.computeIfAbsent(dbKey, key -> {
            log.info("Creating SchemaCache for database: {}", dbKey);

            // Get database configuration from context
            DatabaseConfig config = MultiTenancyContext.getDatabaseConfig();
            if (config == null) {
                throw new IllegalStateException(
                    "No database configuration in context for: " + dbKey
                );
            }

            // Get DataSource for this database
            HikariDataSource dataSource = (HikariDataSource) routingDataSource.getDataSourceByKey(dbKey);
            if (dataSource == null) {
                throw new IllegalStateException(
                    "No DataSource found for database: " + dbKey
                );
            }

            // Create JdbcTemplate for this database
            JdbcTemplate jdbcTemplate = new JdbcTemplate(dataSource);

            // Convert DatabaseConfig to PostgRESTConfig
            PostgRESTConfig postgrestConfig = config.toPostgRESTConfig();

            // Create SchemaCache
            SchemaCache schemaCache = new SchemaCache(jdbcTemplate, postgrestConfig, dbKey);

            log.info("Successfully created SchemaCache for: {}", dbKey);
            return schemaCache;
        });
    }

    /**
     * Reload SchemaCache for a specific database
     *
     * @param dbKey the database key
     */
    public void reloadSchemaCache(String dbKey) {
        log.info("Reloading SchemaCache for: {}", dbKey);

        SchemaCache cache = caches.get(dbKey);

        if (cache != null) {
            // Reload existing cache
            cache.reload();
            log.info("Successfully reloaded SchemaCache for: {}", dbKey);
        } else {
            // Remove from cache to force recreation on next access
            caches.remove(dbKey);
            log.info("Evicted SchemaCache for: {} (will be recreated on next access)", dbKey);
        }
    }

    /**
     * Reload all SchemaCache instances
     */
    public void reloadAllCaches() {
        log.info("Reloading all SchemaCache instances...");

        caches.forEach((dbKey, cache) -> {
            try {
                cache.reload();
                log.info("Reloaded SchemaCache for: {}", dbKey);
            } catch (Exception e) {
                log.error("Failed to reload SchemaCache for: {}", dbKey, e);
            }
        });

        log.info("Completed reloading {} SchemaCache instances", caches.size());
    }

    /**
     * Evict SchemaCache for a database
     * The cache will be recreated on next access
     *
     * @param dbKey the database key
     */
    public void evictCache(String dbKey) {
        log.info("Evicting SchemaCache for: {}", dbKey);
        caches.remove(dbKey);
    }

    /**
     * Evict all caches
     */
    public void evictAllCaches() {
        log.info("Evicting all SchemaCache instances");
        caches.clear();
    }

    /**
     * Check if a SchemaCache exists for a database
     *
     * @param dbKey the database key
     * @return true if cache exists
     */
    public boolean hasCache(String dbKey) {
        return caches.containsKey(dbKey);
    }

    /**
     * Get count of cached schemas
     *
     * @return number of cached SchemaCache instances
     */
    public int getCacheCount() {
        return caches.size();
    }

    /**
     * Get all cached database keys
     *
     * @return set of database keys
     */
    public java.util.Set<String> getCachedDatabases() {
        return caches.keySet();
    }
}
