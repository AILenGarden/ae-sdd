package ai.nubase.mem.repository;

import ai.nubase.mem.config.MemProperties;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowMapper;

import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/**
 * Tests for {@link EntityRepository} focused on the wire-level guarantees that we can verify
 * with a mocked {@link JdbcTemplate}:
 * <ul>
 *   <li>{@code appendLinkedMemory} threads the configured cap into the SQL bindings.</li>
 *   <li>{@code findByExactText} produces the COALESCE-shaped query that matches the
 *       {@code entities_unique_idx} functional index, and handles NULL owner fields.</li>
 *   <li>{@code truncate} silently skips when the table is missing (old tenants).</li>
 * </ul>
 *
 * <p>End-to-end SQL behavior (does the index actually get picked up? Does CASE/cardinality()
 * compute correctly?) needs a real Postgres — covered by future integration tests.
 */
class EntityRepositoryTest {

    private JdbcTemplate jdbc;
    private MemProperties props;
    private EntityRepository repo;

    @BeforeEach
    void setUp() {
        jdbc = mock(JdbcTemplate.class);
        props = new MemProperties();
        repo = new EntityRepository(jdbc, props);
    }

    @Test
    void appendLinkedMemory_passesConfiguredCapAsSecondBinding() {
        UUID entityId = UUID.randomUUID();
        UUID memoryId = UUID.randomUUID();
        props.getEntity().setMaxLinkedMemoryIds(42);

        repo.appendLinkedMemory(entityId, memoryId);

        ArgumentCaptor<String> sqlCap = ArgumentCaptor.forClass(String.class);
        ArgumentCaptor<Object[]> argCap = ArgumentCaptor.forClass(Object[].class);
        verify(jdbc).update(sqlCap.capture(), (Object[]) argCap.capture());

        assertThat(sqlCap.getValue())
                .contains("cardinality(linked_memory_ids) >= ?")
                .contains("array_append(linked_memory_ids, ?)");
        // Positional bindings: memoryId (for = ANY), cap, memoryId (for array_append), entityId.
        Object[] values = argCap.getValue();
        assertThat(values).containsExactly(memoryId, 42, memoryId, entityId);
    }

    @Test
    void appendLinkedMemory_defaultsTo1000WhenConfigMissing() {
        UUID entityId = UUID.randomUUID();
        UUID memoryId = UUID.randomUUID();
        EntityRepository naked = new EntityRepository(jdbc, null);

        naked.appendLinkedMemory(entityId, memoryId);

        ArgumentCaptor<Object[]> argCap = ArgumentCaptor.forClass(Object[].class);
        verify(jdbc).update(anyString(), (Object[]) argCap.capture());
        assertThat(argCap.getValue()).containsExactly(memoryId, 1000, memoryId, entityId);
    }

    @Test
    @SuppressWarnings({"rawtypes", "unchecked"})
    void findByExactText_usesCoalesceShapeMatchingTheUniqueIndex() {
        UUID userId = UUID.randomUUID();
        when(jdbc.query(anyString(), any(RowMapper.class), (Object[]) any()))
                .thenReturn(java.util.List.of());

        repo.findByExactText("John", "person", userId, "agentA", "runZ");

        ArgumentCaptor<String> sqlCap = ArgumentCaptor.forClass(String.class);
        ArgumentCaptor<Object[]> argCap = ArgumentCaptor.forClass(Object[].class);
        verify(jdbc).query(sqlCap.capture(), any(RowMapper.class), (Object[]) argCap.capture());

        // Index shape: lower(text) + COALESCE on every owner field.
        String sql = sqlCap.getValue();
        assertThat(sql).contains("lower(text) = lower(?)");
        assertThat(sql).contains("COALESCE(entity_type, '') = ?");
        assertThat(sql).contains("COALESCE(user_id::text, '') = ?");
        assertThat(sql).contains("COALESCE(agent_id, '') = ?");
        assertThat(sql).contains("COALESCE(run_id, '') = ?");

        // Bound in the same order as the SQL: text, type, userId-as-text, agent, run.
        assertThat(argCap.getValue())
                .containsExactly("John", "person", userId.toString(), "agentA", "runZ");
    }

    @Test
    @SuppressWarnings({"rawtypes", "unchecked"})
    void findByExactText_nullOwnerFieldsBindEmptyStrings() {
        when(jdbc.query(anyString(), any(RowMapper.class), (Object[]) any()))
                .thenReturn(java.util.List.of());

        repo.findByExactText("Tokyo", null, null, null, null);

        ArgumentCaptor<Object[]> argCap = ArgumentCaptor.forClass(Object[].class);
        verify(jdbc).query(anyString(), any(RowMapper.class), (Object[]) argCap.capture());

        // null → "" so they participate in the index expression on every column.
        assertThat(argCap.getValue()).containsExactly("Tokyo", "", "", "", "");
    }

    @Test
    void truncate_swallowsBadSqlGrammar_forTenantsWithoutTable() {
        // Simulate "relation does not exist" — the reset path must not propagate it.
        org.mockito.Mockito.doThrow(new org.springframework.jdbc.BadSqlGrammarException(
                "TRUNCATE", "TRUNCATE TABLE mem.entities",
                new java.sql.SQLException("relation \"mem.entities\" does not exist")))
                .when(jdbc).execute(anyString());

        // Should not throw — silent skip.
        repo.truncate();
    }
}
