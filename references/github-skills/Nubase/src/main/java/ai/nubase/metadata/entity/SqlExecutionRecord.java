package ai.nubase.metadata.entity;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.GeneratedValue;
import jakarta.persistence.GenerationType;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.LocalDateTime;

/**
 * SQL execution audit record. Column names are mapped explicitly so JPA-derived
 * query methods (e.g. {@code findByDatabaseKeyOrderByCreatedAtDesc}) generate
 * the correct snake_case SQL regardless of the active naming strategy.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "sql_execution_records")
public class SqlExecutionRecord {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "app_code", length = 255)
    private String appCode;

    @Column(name = "database_key", length = 255)
    private String databaseKey;

    @Column(name = "schema_name", length = 255)
    private String schemaName;

    @Column(name = "sql_query", nullable = false, columnDefinition = "TEXT")
    private String sqlQuery;

    @Column(name = "success", nullable = false)
    private Boolean success;

    @Column(name = "execution_time_ms")
    private Long executionTimeMs;

    @Column(name = "execution_result", columnDefinition = "TEXT")
    private String executionResult;

    @Column(name = "error_message", columnDefinition = "TEXT")
    private String errorMessage;

    @Column(name = "error_stack_trace", columnDefinition = "TEXT")
    private String errorStackTrace;

    @Column(name = "created_at", nullable = false, updatable = false)
    private LocalDateTime createdAt;

    @PrePersist
    protected void onCreate() {
        if (createdAt == null) {
            createdAt = LocalDateTime.now();
        }
    }
}
