package ai.nubase.postgrest.controller;

import ai.nubase.postgrest.schema.PostgreSQLSchemaWatcher;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.HashMap;
import java.util.Map;

/**
 * Monitoring API for PostgreSQL Schema Watcher
 * <p>
 * Provides endpoints to monitor thread pool health and watcher status
 */
@Slf4j
@RestController
@RequestMapping("/admin/schema-watcher")
public class SchemaWatcherMonitorController {

    @Autowired
    private PostgreSQLSchemaWatcher schemaWatcher;

    /**
     * Get thread pool statistics
     * <p>
     * GET /admin/schema-watcher/stats
     */
    @GetMapping("/stats")
    public ResponseEntity<Map<String, Object>> getStats() {
        Map<String, Object> stats = schemaWatcher.getThreadPoolStats();
        stats.put("healthy", schemaWatcher.isHealthy());
        stats.put("timestamp", System.currentTimeMillis());

        log.debug("Schema watcher stats requested: {}", stats);
        return ResponseEntity.ok(stats);
    }

    /**
     * Get health status
     * <p>
     * GET /admin/schema-watcher/health
     */
    @GetMapping("/health")
    public ResponseEntity<Map<String, Object>> getHealth() {
        boolean healthy = schemaWatcher.isHealthy();
        int activeCount = schemaWatcher.getActiveWatcherCount();

        Map<String, Object> health = new HashMap<>();
        health.put("status", healthy ? "healthy" : "degraded");
        health.put("activeWatchers", activeCount);
        health.put("healthy", healthy);

        if (!healthy) {
            log.warn("Schema watcher health check: DEGRADED - {} active watchers", activeCount);
        }

        return ResponseEntity.ok(health);
    }

    /**
     * Get active watcher count
     * <p>
     * GET /admin/schema-watcher/count
     */
    @GetMapping("/count")
    public ResponseEntity<Map<String, Object>> getCount() {
        int count = schemaWatcher.getActiveWatcherCount();

        Map<String, Object> result = new HashMap<>();
        result.put("activeWatchers", count);

        return ResponseEntity.ok(result);
    }
}
