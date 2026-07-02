package ai.nubase.mem.repository;

import ai.nubase.mem.config.MemProperties;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.Mockito.mock;

/**
 * Locks the whitelist behavior in {@link MemoryRepository#resolveFtsConfig()}.
 *
 * <p>The fts-config value is interpolated directly into the BM25 SQL string (Postgres
 * cannot bind text-search config names as prepared-statement parameters), so it's the
 * one user-supplied value that can become a SQL injection vector. The whitelist is the
 * defense — these tests are the safety net.
 */
class MemoryRepositoryFtsConfigTest {

    private MemoryRepository newRepo(String ftsConfig) {
        MemProperties props = new MemProperties();
        props.getSearch().setFtsConfig(ftsConfig);
        return new MemoryRepository(mock(org.springframework.jdbc.core.JdbcTemplate.class),
                new ObjectMapper(), props);
    }

    @Test
    void simpleIsAllowed() {
        assertThat(newRepo("simple").resolveFtsConfig()).isEqualTo("simple");
    }

    @Test
    void englishIsAllowed() {
        assertThat(newRepo("english").resolveFtsConfig()).isEqualTo("english");
    }

    @Test
    void zhparserIsAllowed() {
        assertThat(newRepo("zhparser").resolveFtsConfig()).isEqualTo("zhparser");
    }

    @Test
    void casePreservedAsLower() {
        assertThat(newRepo("ENGLISH").resolveFtsConfig()).isEqualTo("english");
    }

    @Test
    void unknownConfigFallsBackToSimple() {
        assertThat(newRepo("klingon").resolveFtsConfig()).isEqualTo("simple");
    }

    @Test
    void injectionAttemptFallsBackToSimple() {
        assertThat(newRepo("simple'); DROP TABLE memories;--").resolveFtsConfig())
                .isEqualTo("simple");
    }

    @Test
    void nullFallsBackToSimple() {
        assertThat(newRepo(null).resolveFtsConfig()).isEqualTo("simple");
    }
}
