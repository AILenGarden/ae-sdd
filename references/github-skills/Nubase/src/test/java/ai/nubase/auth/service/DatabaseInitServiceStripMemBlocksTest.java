package ai.nubase.auth.service;

import org.junit.jupiter.api.Test;
import org.springframework.core.io.ClassPathResource;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.stream.Collectors;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Pure-unit tests for {@link DatabaseInitService#stripMemBlocks}. No Spring, no DB.
 *
 * <p>The function is the safety net for {@code nubase.mem.enabled=false}: it removes
 * every {@code -- MEM-BEGIN .. -- MEM-END} region from {@code init_roles.sql} so the
 * tenant init script doesn't try to grant on a non-existent {@code mem} schema.
 *
 * <p>Bugs here would silently break tenant initialization, so we cover both the happy
 * path (each known stanza in the real file is removed) and the defensive cases
 * (unbalanced markers fail loudly).
 */
class DatabaseInitServiceStripMemBlocksTest {

    @Test
    void singleBlock_removed() {
        String in = String.join("\n",
                "SELECT 1;",
                "-- MEM-BEGIN: x",
                "GRANT USAGE ON SCHEMA mem TO foo;",
                "-- MEM-END",
                "SELECT 2;");

        String out = DatabaseInitService.stripMemBlocks(in);

        assertThat(out).isEqualTo("SELECT 1;\nSELECT 2;");
    }

    @Test
    void multipleBlocks_allRemoved_outsideContentPreserved() {
        String in = String.join("\n",
                "BEFORE;",
                "-- MEM-BEGIN: a",
                "DROP MEM A;",
                "-- MEM-END",
                "MIDDLE;",
                "-- MEM-BEGIN: b",
                "DROP MEM B;",
                "MORE MEM B;",
                "-- MEM-END",
                "AFTER;");

        String out = DatabaseInitService.stripMemBlocks(in);

        assertThat(out).isEqualTo("BEFORE;\nMIDDLE;\nAFTER;");
    }

    @Test
    void noBlocks_returnsInputUnchanged() {
        String in = "SELECT 1;\nSELECT 2;";
        assertThat(DatabaseInitService.stripMemBlocks(in)).isEqualTo(in);
    }

    @Test
    void leadingWhitespaceOnMarker_isAccepted() {
        String in = String.join("\n",
                "BEFORE;",
                "    -- MEM-BEGIN: indented",
                "X;",
                "  -- MEM-END",
                "AFTER;");

        assertThat(DatabaseInitService.stripMemBlocks(in)).isEqualTo("BEFORE;\nAFTER;");
    }

    @Test
    void markerInsideStringLiteral_isNotTreatedAsMarker() {
        // A marker must START the line (after stripLeading). A comment substring in the
        // middle of a SQL value would not match, but let's lock the contract just in case.
        String in = String.join("\n",
                "INSERT INTO x VALUES ('look -- MEM-BEGIN: trick');",
                "SELECT 1;");

        assertThat(DatabaseInitService.stripMemBlocks(in)).isEqualTo(in);
    }

    @Test
    void unbalancedBegin_throwsClearly() {
        String in = String.join("\n",
                "-- MEM-BEGIN: oops",
                "X;");
        assertThatThrownBy(() -> DatabaseInitService.stripMemBlocks(in))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("Unclosed");
    }

    @Test
    void unbalancedEnd_throwsClearly() {
        String in = String.join("\n",
                "X;",
                "-- MEM-END");
        assertThatThrownBy(() -> DatabaseInitService.stripMemBlocks(in))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("without a matching");
    }

    @Test
    void nestedBegin_throwsClearly() {
        String in = String.join("\n",
                "-- MEM-BEGIN: outer",
                "-- MEM-BEGIN: inner",
                "-- MEM-END",
                "-- MEM-END");
        assertThatThrownBy(() -> DatabaseInitService.stripMemBlocks(in))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("Nested");
    }

    /**
     * Integration check against the real {@code init_roles.sql} on the classpath.
     *
     * <p>If someone adds a new {@code mem.*} GRANT/POLICY but forgets to wrap it in
     * {@code MEM-BEGIN/MEM-END}, this test will fail — the stripped output should never
     * mention {@code mem} when the feature is disabled.
     */
    @Test
    void realInitRolesSql_strippedOutputHasNoMemReferences() throws Exception {
        ClassPathResource resource = new ClassPathResource("db/supabase/init_roles.sql");
        String sql;
        try (BufferedReader r = new BufferedReader(
                new InputStreamReader(resource.getInputStream(), StandardCharsets.UTF_8))) {
            sql = r.lines().collect(Collectors.joining("\n"));
        }

        String stripped = DatabaseInitService.stripMemBlocks(sql);

        // Stripped output must not reference the mem schema in any DDL — every mem stanza
        // is supposed to be inside MEM-BEGIN/MEM-END blocks.
        assertThat(stripped)
                .doesNotContainIgnoringCase("SCHEMA mem")
                .doesNotContainIgnoringCase("ON mem.")
                .doesNotContainIgnoringCase("mem.memories")
                .doesNotContainIgnoringCase("mem.memory_history")
                .doesNotContainIgnoringCase("mem.entities")
                .doesNotContainIgnoringCase("mem.session_messages");

        // And the file's non-mem content must still be present.
        assertThat(stripped).contains("GRANT USAGE ON SCHEMA auth");
        assertThat(stripped).contains("GRANT USAGE ON SCHEMA storage");
        assertThat(stripped).contains("CREATE EVENT TRIGGER pgrst_watch");
    }
}
