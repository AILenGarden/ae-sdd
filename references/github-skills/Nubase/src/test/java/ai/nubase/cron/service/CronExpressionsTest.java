package ai.nubase.cron.service;

import org.junit.jupiter.api.Test;

import java.time.Instant;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class CronExpressionsTest {

    @Test
    void fiveFieldCrontabIsNormalizedWithSecondsField() {
        assertThat(CronExpressions.normalize("*/5 * * * *")).isEqualTo("0 */5 * * * *");
        assertThat(CronExpressions.normalize("  30 3 * * 1 ")).isEqualTo("0 30 3 * * 1");
    }

    @Test
    void sixFieldSpringFormPassesThrough() {
        assertThat(CronExpressions.normalize("10 */5 * * * *")).isEqualTo("10 */5 * * * *");
    }

    @Test
    void invalidExpressionsAreRejected() {
        assertThatThrownBy(() -> CronExpressions.normalize("not a cron")).isInstanceOf(IllegalArgumentException.class);
        assertThatThrownBy(() -> CronExpressions.normalize("99 * * * *")).isInstanceOf(IllegalArgumentException.class);
        assertThatThrownBy(() -> CronExpressions.normalize(null)).isInstanceOf(IllegalArgumentException.class);
        assertThat(CronExpressions.isValid("* * * * *")).isTrue();
        assertThat(CronExpressions.isValid("bogus")).isFalse();
    }

    @Test
    void nextComputesTheFollowingOccurrenceInUtc() {
        Instant from = Instant.parse("2026-06-11T10:02:15Z");
        assertThat(CronExpressions.next("*/5 * * * *", from)).isEqualTo(Instant.parse("2026-06-11T10:05:00Z"));
        assertThat(CronExpressions.next("0 0 * * *", from)).isEqualTo(Instant.parse("2026-06-12T00:00:00Z"));
    }
}
