package ai.nubase.postgrest.multidb;

import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.postgrest.event.DataSourceCreatedEvent;
import ai.nubase.postgrest.event.DataSourceRemovedEvent;
import com.zaxxer.hikari.HikariConfig;
import com.zaxxer.hikari.HikariDataSource;
import com.zaxxer.hikari.HikariPoolMXBean;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.ApplicationEventPublisher;
import org.springframework.jdbc.datasource.lookup.AbstractRoutingDataSource;
import org.springframework.lang.Nullable;

import javax.sql.DataSource;
import java.sql.Connection;
import java.sql.SQLException;
import java.time.Instant;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Routing DataSource that dynamically selects the target DataSource
 * based on the current database context (from ThreadLocal)
 *
 * This class extends Spring's AbstractRoutingDataSource to provide
 * multi-tenant database routing and lifecycle management capabilities.
 *
 * Responsibilities:
 * - Dynamic routing based on ThreadLocal context
 * - DataSource lifecycle management (create, shutdown, reload)
 * - Health checks and monitoring
 * - Connection pool statistics
 */
@Slf4j
public class RoutingDataSource extends AbstractRoutingDataSource {

    private final Map<Object, Object> targetDataSourceMap = new ConcurrentHashMap<>();
    private final Map<String, DatabaseConfig> dataSourceConfigs = new ConcurrentHashMap<>();
    private final Map<String, Instant> lastAccessTimeMap = new ConcurrentHashMap<>();
    private DataSource defaultTargetDataSource;

    // Event publisher for DataSource lifecycle events
    @Autowired
    private ApplicationEventPublisher eventPublisher;

    @Value("${pgrst.multidb.max-total-connections:200}")
    private int maxTotalConnections = 300;

    // DataSource lifecycle configuration
    private static final long IDLE_TIMEOUT_MINUTES = 1; // 1 hour idle timeout
    private static final long CLEANUP_INTERVAL_MS = 600000; // 10 minutes

    // Cleanup daemon thread
    private volatile Thread cleanupThread;
    private volatile boolean cleanupRunning = false;

    public RoutingDataSource() {
        super();
        // Set the target datasources map
        super.setTargetDataSources(targetDataSourceMap);
    }

    /**
     * Set the default DataSource
     * <p>
     * Used during Hibernate startup initialization, when there is no request context yet.
     * Typically set to the metadata database or an arbitrary tenant database.
     *
     * @param defaultDataSource the default DataSource
     */
    public void setDefaultTargetDataSource(DataSource defaultDataSource) {
        this.defaultTargetDataSource = defaultDataSource;
        super.setDefaultTargetDataSource(defaultDataSource);
        log.info("Default target DataSource configured for Hibernate initialization");

        // Start cleanup daemon thread
        startCleanupThread();
    }

    /**
     * Start the cleanup daemon thread
     */
    private synchronized void startCleanupThread() {
        if (cleanupThread != null && cleanupThread.isAlive()) {
            log.debug("Cleanup thread already running");
            return;
        }

        cleanupRunning = true;
        cleanupThread = new Thread(() -> {
            Thread.currentThread().setName("datasource-cleanup-daemon");
            log.info("DataSource cleanup daemon thread started (interval: {} ms)", CLEANUP_INTERVAL_MS);

            while (cleanupRunning) {
                try {
                    // Wait for cleanup interval
                    Thread.sleep(CLEANUP_INTERVAL_MS);

                    // Perform cleanup
                    if (cleanupRunning) {  // Check again after sleep
                        cleanupIdleDataSources();
                    }

                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    log.info("DataSource cleanup thread interrupted");
                    break;
                } catch (Exception e) {
                    log.error("Error in DataSource cleanup thread", e);
                    // Continue running despite errors
                }
            }

            log.info("DataSource cleanup daemon thread stopped");
        });

        cleanupThread.setDaemon(true);  // Daemon thread won't prevent JVM shutdown
        cleanupThread.setPriority(Thread.MIN_PRIORITY);  // Low priority
        cleanupThread.start();

        log.info("DataSource cleanup daemon thread initialized");
    }

    /**
     * Stop the cleanup daemon thread
     */
    public synchronized void stopCleanupThread() {
        if (cleanupThread != null && cleanupThread.isAlive()) {
            log.info("Stopping DataSource cleanup daemon thread");
            cleanupRunning = false;
            cleanupThread.interrupt();

            try {
                cleanupThread.join(5000);  // Wait up to 5 seconds
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                log.warn("Interrupted while waiting for cleanup thread to stop");
            }
        }
    }

    /**
     * Determine the current lookup key based on ThreadLocal database context
     *
     * @return the database key from MultiTenancyContext
     */
    @Nullable
    @Override
    protected Object determineCurrentLookupKey() {
        String dbKey = MultiTenancyContext.getDatabaseKey();

        if (dbKey == null) {
            // During application startup, it is normal to have no context during Hibernate initialization
            // The default DataSource (metadata database) will be used in this case
            log.debug("No database context set, will use default DataSource. " +
                    "This is normal during application startup or if request bypassed UnifiedMultiTenancyFilter.");
            return null;
        }

        log.trace("Routing to database: {}", dbKey);
        return dbKey;
    }

    /**
     * Dynamically add a new DataSource to the routing map
     *
     * @param dbKey the database key
     * @param dataSource the DataSource instance
     */
    public void addDataSource(String dbKey, DataSource dataSource) {
        if (dbKey == null || dbKey.isBlank()) {
            throw new IllegalArgumentException("Database key cannot be null or blank");
        }

        if (dataSource == null) {
            throw new IllegalArgumentException("DataSource cannot be null");
        }

        log.info("Adding DataSource for database: {}", dbKey);

        targetDataSourceMap.put(dbKey, dataSource);

        // Record initial access time
        lastAccessTimeMap.put(dbKey, Instant.now());

        // Refresh the routing datasource
        super.setTargetDataSources(targetDataSourceMap);
        super.afterPropertiesSet();

        log.info("Successfully added DataSource for database: {}", dbKey);
    }

    /**
     * Dynamically remove a DataSource from the routing map
     * Also closes the HikariDataSource and stops schema watcher if applicable
     *
     * @param dbKey the database key
     * @return the removed DataSource, or null if not found
     */
    public DataSource removeDataSource(String dbKey) {
        log.info("Removing DataSource for database: {}", dbKey);

        // Publish event to notify schema watcher and other listeners
        eventPublisher.publishEvent(new DataSourceRemovedEvent(this, dbKey));

        Object removed = targetDataSourceMap.remove(dbKey);
        dataSourceConfigs.remove(dbKey);
        lastAccessTimeMap.remove(dbKey); // Clean up access time tracking

        if (removed != null) {
            // Close HikariDataSource if applicable
            if (removed instanceof HikariDataSource) {
                HikariDataSource hikariDS = (HikariDataSource) removed;
                try {
                    if (!hikariDS.isClosed()) {
                        hikariDS.close();
                        log.info("Closed HikariDataSource for: {}", dbKey);
                    }
                } catch (Exception e) {
                    log.error("Error closing HikariDataSource for {}", dbKey, e);
                }
            }

            // Refresh the routing datasource
            super.setTargetDataSources(targetDataSourceMap);
            super.afterPropertiesSet();

            log.info("Successfully removed DataSource for database: {}", dbKey);
            return (DataSource) removed;
        } else {
            log.warn("No DataSource found to remove for database: {}", dbKey);
            return null;
        }
    }

    /**
     * Get a specific DataSource by database key
     *
     * @param dbKey the database key
     * @return the DataSource, or null if not found
     */
    public DataSource getDataSourceByKey(String dbKey) {
        if (dbKey == null) {
            return null;
        }
        return (DataSource) targetDataSourceMap.get(dbKey);
    }

    /**
     * Check if a DataSource exists for the given database key
     *
     * @param dbKey the database key
     * @return true if DataSource exists
     */
    public boolean hasDataSource(String dbKey) {
        return targetDataSourceMap.containsKey(dbKey);
    }

    /**
     * Get the number of registered DataSources
     *
     * @return count of DataSources
     */
    public int getDataSourceCount() {
        return targetDataSourceMap.size();
    }

    /**
     * Get all registered database keys
     *
     * @return set of database keys
     */
    public java.util.Set<String> getDataSourceKeys() {
        return targetDataSourceMap.keySet().stream()
                .map(Object::toString)
                .collect(java.util.stream.Collectors.toSet());
    }

    /**
     * Override to provide better error messages when DataSource not found
     */
    @Override
    protected DataSource determineTargetDataSource() {
        try {
            return super.determineTargetDataSource();
        } catch (IllegalStateException e) {
            String dbKey = MultiTenancyContext.getDatabaseKey();
            throw new IllegalStateException(
                "No DataSource found for database key: " + dbKey +
                ". Available databases: " + getDataSourceKeys(), e
            );
        }
    }

    // ==================== DataSource Lifecycle Management ====================

    /**
     * Initialize a new DataSource for a database and register it
     *
     * @param config the database configuration
     * @return the initialized HikariDataSource
     * @throws IllegalStateException if global connection limit would be exceeded
     */
    public synchronized HikariDataSource initializeDataSource(DatabaseConfig config) {
        String dbKey = config.getDbKey();

        // Check if already exists
        if (hasDataSource(dbKey)) {
            log.debug("DataSource already exists for: {}", dbKey);
            return (HikariDataSource) getDataSourceByKey(dbKey);
        }

        // Check global connection limit
        int currentTotalConnections = getAllPoolsConnectionCount();
        int newPoolSize = config.getPoolSize() != null ? config.getPoolSize() : 10;

        if (currentTotalConnections + newPoolSize > maxTotalConnections) {
            throw new IllegalStateException(
                String.format("Cannot create datasource for %s: would exceed max total connections " +
                    "(current: %d, new pool: %d, max: %d)",
                    dbKey, currentTotalConnections, newPoolSize, maxTotalConnections)
            );
        }

        log.info("Initializing DataSource for database: {}", dbKey);

        try {
            // Convert DatabaseConfig to HikariConfig
            HikariConfig hikariConfig = config.toHikariConfig();

            // Create DataSource
            HikariDataSource dataSource = new HikariDataSource(hikariConfig);

            // Store config and register DataSource
            dataSourceConfigs.put(dbKey, config);
            addDataSource(dbKey, dataSource);

            // Publish event to notify schema watcher and other listeners
            eventPublisher.publishEvent(new DataSourceCreatedEvent(this, dbKey, config));

            log.info("Successfully initialized and registered DataSource for: {} (pool size: {})",
                dbKey, hikariConfig.getMaximumPoolSize());

            return dataSource;

        } catch (Exception e) {
            log.error("Failed to initialize DataSource for: {}", dbKey, e);
            throw new IllegalStateException("Failed to initialize DataSource for: " + dbKey, e);
        }
    }

    /**
     * Get or create a DataSource
     *
     * @param dbKey the database key
     * @param config the database configuration (used if datasource doesn't exist)
     * @return the DataSource
     */
    public HikariDataSource getOrCreateDataSource(String dbKey, DatabaseConfig config) {
        HikariDataSource dataSource = (HikariDataSource) getDataSourceByKey(dbKey);

        if (dataSource == null || dataSource.isClosed()) {
            synchronized (this) {
                // Double-check locking
                dataSource = (HikariDataSource) getDataSourceByKey(dbKey);
                if (dataSource == null || dataSource.isClosed()) {
                    dataSource = initializeDataSource(config);
                }
            }
        }

        return dataSource;
    }

    /**
     * Shutdown and remove a DataSource
     *
     * @param dbKey the database key
     */
    public synchronized void shutdownDataSource(String dbKey) {
        log.info("Shutting down DataSource for: {}", dbKey);

        // Remove from routing map (returns the removed DataSource)
        HikariDataSource dataSource = (HikariDataSource) removeDataSource(dbKey);
        dataSourceConfigs.remove(dbKey);

        if (dataSource != null) {
            try {
                // Close the pool gracefully
                dataSource.close();
                log.info("Successfully shut down DataSource for: {}", dbKey);
            } catch (Exception e) {
                log.error("Error shutting down DataSource for: {}", dbKey, e);
            }
        } else {
            log.warn("No DataSource found to shut down for: {}", dbKey);
        }
    }

    /**
     * Reload a DataSource with new configuration
     *
     * @param config the new database configuration
     */
    public synchronized void reloadDataSource(DatabaseConfig config) {
        String dbKey = config.getDbKey();
        log.info("Reloading DataSource for: {}", dbKey);

        // Shutdown existing if present
        shutdownDataSource(dbKey);

        // Initialize new
        initializeDataSource(config);

        log.info("Successfully reloaded DataSource for: {}", dbKey);
    }

    /**
     * Shutdown all DataSources
     * Called during application shutdown
     */
    public synchronized void shutdownAll() {
        log.info("Shutting down all DataSources...");

        // Get all database keys and shutdown each one
        getDataSourceKeys().forEach(this::shutdownDataSource);

        log.info("All DataSources shut down");
    }

    // ==================== Health Check & Monitoring ====================

    /**
     * Perform health check on a database
     *
     * @param dbKey the database key
     * @return true if healthy, false otherwise
     */
    public boolean healthCheck(String dbKey) {
        HikariDataSource dataSource = (HikariDataSource) getDataSourceByKey(dbKey);

        if (dataSource == null || dataSource.isClosed()) {
            log.warn("Health check failed for {}: DataSource not available", dbKey);
            return false;
        }

        try (Connection connection = dataSource.getConnection()) {
            // Execute simple query
            boolean isValid = connection.isValid(5); // 5 second timeout

            if (isValid) {
                log.debug("Health check passed for: {}", dbKey);
            } else {
                log.warn("Health check failed for {}: connection not valid", dbKey);
            }

            return isValid;

        } catch (SQLException e) {
            log.error("Health check failed for {}: {}", dbKey, e.getMessage());
            return false;
        }
    }

    /**
     * Pre-warm a connection pool
     * Creates connections up to minimum idle
     *
     * @param dbKey the database key
     */
    public void prewarmPool(String dbKey) {
        HikariDataSource dataSource = (HikariDataSource) getDataSourceByKey(dbKey);
        if (dataSource == null || dataSource.isClosed()) {
            log.warn("Cannot prewarm pool for {}: DataSource not available", dbKey);
            return;
        }

        log.info("Pre-warming connection pool for: {}", dbKey);

        try (Connection conn = dataSource.getConnection()) {
            conn.prepareStatement("SELECT 1").execute();
            log.info("Successfully pre-warmed pool for: {}", dbKey);
        } catch (SQLException e) {
            log.error("Failed to pre-warm pool for {}: {}", dbKey, e.getMessage());
        }
    }

    // ==================== Connection Pool Statistics ====================

    /**
     * Get total connection count across all pools
     *
     * @return total active + idle connections
     */
    public int getAllPoolsConnectionCount() {
        return getDataSourceKeys().stream()
            .mapToInt(dbKey -> {
                HikariDataSource ds = (HikariDataSource) getDataSourceByKey(dbKey);
                if (ds != null && !ds.isClosed()) {
                    return ds.getHikariPoolMXBean().getTotalConnections();
                }
                return 0;
            })
            .sum();
    }


    /**
     * Get pool statistics for a database
     *
     * @param dbKey the database key
     * @return pool statistics as formatted string
     */
    public String getPoolStats(String dbKey) {
        HikariDataSource dataSource = (HikariDataSource) getDataSourceByKey(dbKey);
        if (dataSource == null || dataSource.isClosed()) {
            return "No pool found for: " + dbKey;
        }

        var mxBean = dataSource.getHikariPoolMXBean();
        return String.format(
            "Pool[%s] - Total: %d, Active: %d, Idle: %d, Waiting: %d",
            dbKey,
            mxBean.getTotalConnections(),
            mxBean.getActiveConnections(),
            mxBean.getIdleConnections(),
            mxBean.getThreadsAwaitingConnection()
        );
    }

    /**
     * Get database configuration
     *
     * @param dbKey the database key
     * @return the configuration, or null if not found
     */
    public DatabaseConfig getDatabaseConfig(String dbKey) {
        return dataSourceConfigs.get(dbKey);
    }

    // ==================== DataSource Access Tracking & Lifecycle ====================

    /**
     * Record access to a DataSource
     * Should be called on every request that uses a specific database
     *
     * @param dbKey the database key
     */
    public void recordAccess(String dbKey) {
        if (dbKey != null && hasDataSource(dbKey)) {
            lastAccessTimeMap.put(dbKey, Instant.now());
            log.trace("Recorded access for database: {}", dbKey);
        }
    }

    /**
     * Get last access time for a DataSource
     *
     * @param dbKey the database key
     * @return last access time, or null if not found
     */
    public Instant getLastAccessTime(String dbKey) {
        return lastAccessTimeMap.get(dbKey);
    }

    /**
     * Get idle time in hours for a DataSource
     *
     * @param dbKey the database key
     * @return idle hours, or -1 if not found
     */
    public long getIdleHours(String dbKey) {
        Instant lastAccess = lastAccessTimeMap.get(dbKey);
        if (lastAccess == null) {
            return -1;
        }
        return java.time.Duration.between(lastAccess, Instant.now()).toHours();
    }

    /**
     * Cleanup idle DataSources
     * <p>
     * Called by the cleanup daemon thread every 10 minutes.
     * Removes DataSources idle for more than 1 hour with 0 active connections.
     * <p>
     * Note: This method is NOT annotated with @Scheduled.
     * Instead, it's called by a dedicated daemon thread to avoid dependency on Spring scheduling.
     */
    public synchronized void cleanupIdleDataSources() {
        log.info("Starting idle DataSource cleanup check");

        Instant now = Instant.now();
        int totalCount = targetDataSourceMap.size();
        int removedCount = 0;

        log.info("Total DataSources before cleanup: {}", totalCount);
        // Iterate over a copy of keys to avoid ConcurrentModificationException
        for (String dbKey : getDataSourceKeys()) {
            try {
                if (shouldEvictDataSource(dbKey, now)) {
                    log.info("Evicting idle DataSource: {} (idle for {} hours)",
                            dbKey, getIdleHours(dbKey));

                    removeDataSource(dbKey);
                    removedCount++;
                }
            } catch (Exception e) {
                log.error("Error during cleanup check for DataSource: {}", dbKey, e);
            }
        }

        if (removedCount > 0) {
            log.info("Idle DataSource cleanup completed: removed {}/{} DataSources",
                    removedCount, totalCount);
        } else {
            log.debug("Idle DataSource cleanup completed: no DataSources removed (total: {})", totalCount);
        }
    }

    /**
     * Determine if a DataSource should be evicted
     *
     * @param dbKey the database key
     * @param now current time
     * @return true if should evict
     */
    private boolean shouldEvictDataSource(String dbKey, Instant now) {
        // Get last access time
        Instant lastAccess = lastAccessTimeMap.get(dbKey);
        if (lastAccess == null) {
            // No access record, mark current time and don't evict
            lastAccessTimeMap.put(dbKey, now);
            return false;
        }

        // Calculate idle time
        long idleHours = java.time.Duration.between(lastAccess, now).toMinutes();

        // Must be idle for at least IDLE_TIMEOUT_MINUTES
        if (idleHours < IDLE_TIMEOUT_MINUTES) {
            return false;
        }

        // Get DataSource
        DataSource dataSource = getDataSourceByKey(dbKey);
        if (!(dataSource instanceof HikariDataSource)) {
            log.warn("DataSource {} is not HikariDataSource, skipping eviction", dbKey);
            return false;
        }

        HikariDataSource hikariDS = (HikariDataSource) dataSource;

        // Check if DataSource is already closed
        if (hikariDS.isClosed()) {
            log.warn("DataSource {} is already closed, removing from map", dbKey);
            return true;
        }

        // Check pool status
        try {
            HikariPoolMXBean poolMXBean = hikariDS.getHikariPoolMXBean();
            if (poolMXBean == null) {
                log.warn("Cannot get pool MXBean for {}, skipping eviction", dbKey);
                return false;
            }

            int activeConnections = poolMXBean.getActiveConnections();
            int totalConnections = poolMXBean.getTotalConnections();

            log.debug("DataSource {} stats: idle={}h, active={}, total={}",
                    dbKey, idleHours, activeConnections, totalConnections);

            // Only evict if there are NO active connections
            if (activeConnections > 0) {
                log.debug("DataSource {} has {} active connections, not evicting",
                        dbKey, activeConnections);
                return false;
            }

            // Evict if idle long enough and no active connections
            log.info("DataSource {} eligible for eviction: idle for {} hours, 0 active connections",
                    dbKey, idleHours);
            return true;

        } catch (Exception e) {
            log.error("Error checking pool status for {}", dbKey, e);
            return false;
        }
    }

    /**
     * Get comprehensive statistics for all DataSources
     *
     * @return map of database key to statistics
     */
    public Map<String, DataSourceStats> getAllDataSourceStats() {
        Map<String, DataSourceStats> stats = new ConcurrentHashMap<>();
        Instant now = Instant.now();

        for (String dbKey : getDataSourceKeys()) {
            try {
                DataSource ds = getDataSourceByKey(dbKey);
                if (ds instanceof HikariDataSource) {
                    HikariDataSource hikariDS = (HikariDataSource) ds;

                    if (!hikariDS.isClosed()) {
                        HikariPoolMXBean pool = hikariDS.getHikariPoolMXBean();
                        if (pool != null) {
                            Instant lastAccess = lastAccessTimeMap.getOrDefault(dbKey, now);
                            long idleHours = java.time.Duration.between(lastAccess, now).toHours();

                            stats.put(dbKey, new DataSourceStats(
                                    dbKey,
                                    pool.getActiveConnections(),
                                    pool.getIdleConnections(),
                                    pool.getTotalConnections(),
                                    pool.getThreadsAwaitingConnection(),
                                    idleHours,
                                    lastAccess
                            ));
                        }
                    }
                }
            } catch (Exception e) {
                log.error("Error getting stats for DataSource: {}", dbKey, e);
            }
        }

        return stats;
    }

    /**
     * Force eviction of a specific DataSource
     * Useful for admin operations
     *
     * @param dbKey the database key
     * @return true if evicted, false if not found
     */
    public boolean forceEvict(String dbKey) {
        log.warn("Force evicting DataSource: {}", dbKey);
        DataSource removed = removeDataSource(dbKey);
        return removed != null;
    }

    /**
     * DataSource statistics record
     */
    public record DataSourceStats(
            String databaseKey,
            int activeConnections,
            int idleConnections,
            int totalConnections,
            int threadsAwaiting,
            long idleHours,
            Instant lastAccessTime
    ) {
        @Override
        public String toString() {
            return String.format("DataSourceStats[db=%s, active=%d, idle=%d, total=%d, idleHours=%d]",
                    databaseKey, activeConnections, idleConnections, totalConnections, idleHours);
        }
    }

}
