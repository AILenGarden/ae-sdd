package ai.nubase.postgrest.schema;

import ai.nubase.postgrest.event.DataSourceCreatedEvent;
import ai.nubase.postgrest.event.DataSourceRemovedEvent;
import ai.nubase.postgrest.multidb.DatabaseConfig;
import ai.nubase.postgrest.multidb.SchemaCacheManager;
import lombok.extern.slf4j.Slf4j;
import org.postgresql.PGConnection;
import org.postgresql.PGNotification;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.event.EventListener;
import org.springframework.stereotype.Component;

import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.Statement;
import java.util.Map;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * PostgreSQL Schema Change Watcher
 * <p>
 * Listens for PostgreSQL NOTIFY events on the 'pgrst' channel
 * and automatically refreshes schema cache when DDL changes occur.
 * <p>
 * Architecture:
 * - One listener thread per active database
 * - Uses INDEPENDENT JDBC connections (NOT from connection pool)
 * - Event-driven integration with RoutingDataSource (no circular dependency)
 * - Automatically stops when DataSource is removed
 *
 * @author nubase
 * @since 2025-01-05
 */
@Slf4j
@Component
public class PostgreSQLSchemaWatcher {

    private final SchemaCacheManager schemaCacheManager;

    // Map: database_key -> listener thread
    private final Map<String, ListenerThread> activeListeners = new ConcurrentHashMap<>();

    // Executor for listener threads
    private final ExecutorService executorService;

    // Configuration: maximum concurrent schema watchers
    @Value("${pgrst.schema-watcher.max-threads:50}")
    private int maxWatcherThreads = 50;

    @Value("${pgrst.schema-watcher.queue-capacity:100}")
    private int queueCapacity = 100;

    // Thread counter for naming
    private static final AtomicInteger threadCounter = new AtomicInteger(0);

    public PostgreSQLSchemaWatcher(SchemaCacheManager schemaCacheManager) {
        this.schemaCacheManager = schemaCacheManager;

        // Create bounded thread pool to prevent resource exhaustion
        // Core threads: Start with 10, grow to max as needed
        // Max threads: Configurable via property (default 50)
        // Queue: Bounded to prevent unbounded growth
        // Rejected: CallerRunsPolicy - run in caller thread if pool is full
        this.executorService = new ThreadPoolExecutor(
            10,                           // corePoolSize: initial threads
            maxWatcherThreads,           // maximumPoolSize: max concurrent watchers
            60L,                         // keepAliveTime: idle thread timeout
            TimeUnit.SECONDS,
            new LinkedBlockingQueue<>(queueCapacity),  // bounded queue
            this::createWatcherThread,    // thread factory
            new ThreadPoolExecutor.CallerRunsPolicy()  // rejection policy
        );

        log.info("PostgreSQL Schema Watcher initialized (event-driven mode, max threads: {})", maxWatcherThreads);
    }

    /**
     * Create daemon thread for schema watching
     */
    private Thread createWatcherThread(Runnable r) {
        Thread t = new Thread(r);
        t.setDaemon(true);  // Won't prevent JVM shutdown
        t.setName("schema-watcher-" + threadCounter.incrementAndGet());
        t.setPriority(Thread.NORM_PRIORITY - 1);  // Slightly lower priority
        return t;
    }

    /**
     * Listen for DataSource creation events
     */
    @EventListener
    public void onDataSourceCreated(DataSourceCreatedEvent event) {
        String databaseKey = event.getDatabaseKey();
        DatabaseConfig config = event.getConfig();
        log.info("Received DataSourceCreatedEvent for: {}", databaseKey);
        startWatching(databaseKey, config);
    }

    /**
     * Listen for DataSource removal events
     */
    @EventListener
    public void onDataSourceRemoved(DataSourceRemovedEvent event) {
        String databaseKey = event.getDatabaseKey();

        log.info("Received DataSourceRemovedEvent for: {}", databaseKey);
        stopWatching(databaseKey);
        schemaCacheManager.evictCache(databaseKey);
    }

    /**
     * Start listening for schema changes on a specific database
     *
     * @param databaseKey the database key
     * @param config the database configuration
     */
    private synchronized void startWatching(String databaseKey, DatabaseConfig config) {
        if (activeListeners.containsKey(databaseKey)) {
            log.debug("Schema watcher already active for: {}", databaseKey);
            return;
        }

        // Check if we're approaching thread pool limits
        int activeCount = activeListeners.size();
        if (activeCount >= maxWatcherThreads * 0.8) {
            log.warn("Schema watcher approaching thread limit: {}/{} active watchers",
                    activeCount, maxWatcherThreads);
        }

        if (activeCount >= maxWatcherThreads) {
            log.error("Schema watcher thread limit reached: {}/{}. Cannot start watcher for: {}",
                    activeCount, maxWatcherThreads, databaseKey);
            log.error("Consider increasing pgrst.schema-watcher.max-threads or reducing active databases");
            return;
        }

        log.info("Starting schema change watcher for: {} ({}/{} active)",
                databaseKey, activeCount + 1, maxWatcherThreads);

        ListenerThread listener = new ListenerThread(databaseKey, config);
        activeListeners.put(databaseKey, listener);

        try {
            executorService.submit(listener);
        } catch (RejectedExecutionException e) {
            log.error("Failed to submit schema watcher for {}: thread pool rejected task", databaseKey, e);
            activeListeners.remove(databaseKey);
        }
    }

    /**
     * Stop watching a specific database
     *
     * @param databaseKey the database key
     */
    private synchronized void stopWatching(String databaseKey) {
        ListenerThread listener = activeListeners.remove(databaseKey);
        if (listener != null) {
            log.info("Stopping schema change watcher for: {}", databaseKey);
            listener.stop();
        }
    }

    /**
     * Check if watching a database
     *
     * @param databaseKey the database key
     * @return true if watching
     */
    public boolean isWatching(String databaseKey) {
        return activeListeners.containsKey(databaseKey);
    }

    /**
     * Stop all watchers (called on application shutdown)
     */
    public synchronized void stopAll() {
        log.info("Stopping all schema change watchers");
        activeListeners.forEach((key, listener) -> listener.stop());
        activeListeners.clear();
        executorService.shutdown();
    }

    /**
     * Get count of active watchers
     */
    public int getActiveWatcherCount() {
        return activeListeners.size();
    }

    /**
     * Get thread pool statistics
     */
    public Map<String, Object> getThreadPoolStats() {
        if (executorService instanceof ThreadPoolExecutor) {
            ThreadPoolExecutor tpe = (ThreadPoolExecutor) executorService;
            Map<String, Object> stats = new ConcurrentHashMap<>();
            stats.put("activeThreads", tpe.getActiveCount());
            stats.put("poolSize", tpe.getPoolSize());
            stats.put("corePoolSize", tpe.getCorePoolSize());
            stats.put("maxPoolSize", tpe.getMaximumPoolSize());
            stats.put("queueSize", tpe.getQueue().size());
            stats.put("queueCapacity", queueCapacity);
            stats.put("completedTasks", tpe.getCompletedTaskCount());
            stats.put("totalTasks", tpe.getTaskCount());
            stats.put("activeWatchers", activeListeners.size());
            return stats;
        }
        return Map.of("activeWatchers", activeListeners.size());
    }

    /**
     * Check if thread pool is healthy (not overloaded)
     */
    public boolean isHealthy() {
        int activeCount = activeListeners.size();
        return activeCount < maxWatcherThreads * 0.9;  // Healthy if < 90% capacity
    }

    /**
     * Listener thread for a single database
     * Uses an INDEPENDENT JDBC connection (NOT from HikariCP pool)
     */
    private class ListenerThread implements Runnable {
        private final String databaseKey;
        private final DatabaseConfig config;
        private volatile boolean running = true;
        private Connection connection;

        public ListenerThread(String databaseKey, DatabaseConfig config) {
            this.databaseKey = databaseKey;
            this.config = config;
        }

        public void stop() {
            log.debug("Stop requested for schema watcher: {}", databaseKey);
            running = false;

            // Close connection immediately to interrupt getNotifications() blocking call
            if (connection != null) {
                try {
                    // Closing the connection will cause getNotifications() to throw exception
                    // or isValid() to return false, allowing the loop to exit quickly
                    connection.close();
                    log.debug("Listener connection closed for: {}", databaseKey);
                } catch (Exception e) {
                    log.warn("Error closing listener connection for {}", databaseKey, e);
                }
            }
        }

        @Override
        public void run() {
            Thread.currentThread().setName("schema-watcher-" + databaseKey);

            while (running) {
                try {
                    // Create INDEPENDENT connection using DriverManager (NOT from pool)
                    // This avoids occupying a connection from HikariCP pool
                    connection = createDirectConnection(config);

                    if (connection == null) {
                        log.warn("Failed to create direct connection for {}, retrying in 30s", databaseKey);
                        Thread.sleep(30000);
                        continue;
                    }

                    PGConnection pgConn = connection.unwrap(PGConnection.class);

                    // Start listening on 'pgrst' channel
                    try (Statement stmt = connection.createStatement()) {
                        stmt.execute("LISTEN pgrst");
                        log.info("Schema watcher active for {} on channel 'pgrst' (independent connection)", databaseKey);
                    }

                    // Poll for notifications
                    // This loop will run continuously until:
                    // 1. stop() is called (sets running=false and closes connection)
                    // 2. Connection becomes invalid
                    // 3. Exception occurs
                    while (running) {
                        // Poll for notifications (1 second timeout for faster stop response)
                        // Using shorter timeout allows faster response to stop() calls
                        PGNotification[] notifications = pgConn.getNotifications(1000);

                        if (notifications != null && notifications.length > 0) {
                            for (PGNotification notification : notifications) {
                                log.info("Received notification on {}: channel={}, message={}",
                                        databaseKey, notification.getName(), notification.getParameter());

                                if ("pgrst".equals(notification.getName())) {
                                    handleSchemaChange(databaseKey);
                                }
                            }
                        }

                        // Periodically check if we should stop (every iteration)
                        if (!running) {
                            log.info("Stop signal received for {}, exiting listener loop", databaseKey);
                            break;
                        }

                        // Check connection health
                        if (!connection.isValid(2)) {
                            log.warn("Connection lost for {}, reconnecting", databaseKey);
                            break;
                        }
                    }

                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    log.info("Schema watcher interrupted for {}", databaseKey);
                    break;
                } catch (Exception e) {
                    // Check if this is due to stop() being called
                    if (!running) {
                        log.info("Schema watcher stopped during operation for {}", databaseKey);
                        break;
                    }

                    log.error("Error in schema watcher for {}", databaseKey, e);

                    // Only retry if still supposed to be running
                    if (running) {
                        log.info("Will retry connection for {} in 30 seconds", databaseKey);
                        try {
                            Thread.sleep(30000); // 30 seconds
                        } catch (InterruptedException ie) {
                            Thread.currentThread().interrupt();
                            break;
                        }
                    }
                } finally {
                    if (connection != null) {
                        try {
                            connection.close();
                        } catch (Exception e) {
                            log.warn("Error closing connection for {}", databaseKey, e);
                        }
                        connection = null;
                    }
                }
            }

            log.info("Schema watcher stopped for: {}", databaseKey);
            activeListeners.remove(databaseKey);
        }

        /**
         * Create a direct JDBC connection using DriverManager
         * This connection is INDEPENDENT from HikariCP pool
         */
        private Connection createDirectConnection(DatabaseConfig config) {
            try {
                String jdbcUrl = config.getJdbcUrl();
                String username = config.getDbUser();
                String password = config.getDbPasswordDecrypted();

                log.debug("Creating direct JDBC connection for schema watcher: {}", databaseKey);

                // Create connection directly via DriverManager (bypassing pool)
                Connection conn = DriverManager.getConnection(jdbcUrl, username, password);

                // Configure connection for long-running LISTEN
                conn.setAutoCommit(true);

                log.debug("Direct connection established for schema watcher: {}", databaseKey);
                return conn;

            } catch (Exception e) {
                log.error("Failed to create direct connection for {}", databaseKey, e);
                return null;
            }
        }

        private void handleSchemaChange(String databaseKey) {
            log.info("Schema change detected for {}, refreshing cache", databaseKey);
            try {
                schemaCacheManager.reloadSchemaCache(databaseKey);
                log.info("Schema cache refreshed successfully for: {}", databaseKey);
            } catch (Exception e) {
                log.error("Failed to refresh schema cache for {}", databaseKey, e);
            }
        }
    }
}
