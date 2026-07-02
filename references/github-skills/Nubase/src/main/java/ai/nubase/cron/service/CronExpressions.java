package ai.nubase.cron.service;

import org.springframework.scheduling.support.CronExpression;

import java.time.Instant;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;

/**
 * Cron parsing helpers. Accepts both the 5-field crontab form Supabase users
 * write (minute-level) and Spring's native 6-field form (with seconds); a
 * 5-field expression is normalized by prepending a "0" seconds field.
 * All schedules evaluate in UTC.
 */
public final class CronExpressions {

    private CronExpressions() {
    }

    public static String normalize(String expression) {
        if (expression == null) {
            throw new IllegalArgumentException("Cron expression is required");
        }
        String trimmed = expression.trim();
        String[] fields = trimmed.split("\\s+");
        String candidate = fields.length == 5 ? "0 " + trimmed : trimmed;
        CronExpression.parse(candidate);
        return candidate;
    }

    public static boolean isValid(String expression) {
        try {
            normalize(expression);
            return true;
        } catch (IllegalArgumentException e) {
            return false;
        }
    }

    public static Instant next(String expression, Instant from) {
        CronExpression cron = CronExpression.parse(normalize(expression));
        ZonedDateTime next = cron.next(ZonedDateTime.ofInstant(from, ZoneOffset.UTC));
        if (next == null) {
            throw new IllegalArgumentException("Cron expression has no future occurrence: " + expression);
        }
        return next.toInstant();
    }
}
