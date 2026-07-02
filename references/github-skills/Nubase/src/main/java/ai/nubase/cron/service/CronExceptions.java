package ai.nubase.cron.service;

import ai.nubase.common.web.ApiStatusException;
import org.springframework.http.HttpStatus;

public final class CronExceptions {

    private CronExceptions() {
    }

    public static class CronException extends ApiStatusException {

        public CronException(HttpStatus status, String code, String message) {
            super(status, code, message);
        }
    }
}
