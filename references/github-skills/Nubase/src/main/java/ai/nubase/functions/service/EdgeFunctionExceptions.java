package ai.nubase.functions.service;

import ai.nubase.common.web.ApiStatusException;
import org.springframework.http.HttpStatus;

public final class EdgeFunctionExceptions {

    private EdgeFunctionExceptions() {
    }

    public static class EdgeFunctionException extends ApiStatusException {

        public EdgeFunctionException(HttpStatus status, String code, String message) {
            super(status, code, message);
        }
    }
}
