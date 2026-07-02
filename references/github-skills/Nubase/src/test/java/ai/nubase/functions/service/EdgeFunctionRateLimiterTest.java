package ai.nubase.functions.service;

import ai.nubase.functions.executor.EdgeFunctionExecutorProperties;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentMatchers;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.script.RedisScript;
import org.springframework.http.HttpStatus;
import org.springframework.test.util.ReflectionTestUtils;

import static ai.nubase.functions.service.EdgeFunctionExceptions.EdgeFunctionException;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyList;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

class EdgeFunctionRateLimiterTest {

    @Test
    void rejectsRequestsAboveProjectLimit() {
        EdgeFunctionExecutorProperties properties = new EdgeFunctionExecutorProperties();
        properties.setPerProjectRpm(1);
        properties.setPerFunctionRpm(0);
        EdgeFunctionRateLimiter limiter = new EdgeFunctionRateLimiter(properties);

        limiter.check("project-a", "hello");

        assertThatThrownBy(() -> limiter.check("project-a", "world"))
                .isInstanceOf(EdgeFunctionException.class)
                .extracting(ex -> ((EdgeFunctionException) ex).status())
                .isEqualTo(HttpStatus.TOO_MANY_REQUESTS);
    }

    @Test
    void disabledLimitsDoNotReject() {
        EdgeFunctionExecutorProperties properties = new EdgeFunctionExecutorProperties();
        properties.setPerProjectRpm(0);
        properties.setPerFunctionRpm(0);
        EdgeFunctionRateLimiter limiter = new EdgeFunctionRateLimiter(properties);

        for (int i = 0; i < 10; i += 1) {
            limiter.check("project-a", "hello");
        }
    }

    @Test
    void rejectsRequestsAboveRedisLimit() {
        EdgeFunctionExecutorProperties properties = new EdgeFunctionExecutorProperties();
        properties.setPerProjectRpm(1);
        properties.setPerFunctionRpm(0);
        EdgeFunctionRateLimiter limiter = new EdgeFunctionRateLimiter(properties);
        StringRedisTemplate redisTemplate = mock(StringRedisTemplate.class);
        when(redisTemplate.execute(ArgumentMatchers.<RedisScript<Long>>any(), anyList(), any()))
                .thenReturn(1L, 2L);
        ReflectionTestUtils.setField(limiter, "redisTemplate", redisTemplate);

        limiter.check("project-a", "hello");

        assertThatThrownBy(() -> limiter.check("project-a", "world"))
                .isInstanceOf(EdgeFunctionException.class)
                .extracting(ex -> ((EdgeFunctionException) ex).status())
                .isEqualTo(HttpStatus.TOO_MANY_REQUESTS);

        // The increment and its expiry run as one atomic script per request.
        verify(redisTemplate, times(2)).execute(ArgumentMatchers.<RedisScript<Long>>any(), anyList(), any());
    }

    @Test
    void redisFailureFallsBackToLocalCounterWithoutDoubleCounting() {
        EdgeFunctionExecutorProperties properties = new EdgeFunctionExecutorProperties();
        properties.setPerProjectRpm(1);
        properties.setPerFunctionRpm(0);
        EdgeFunctionRateLimiter limiter = new EdgeFunctionRateLimiter(properties);
        StringRedisTemplate redisTemplate = mock(StringRedisTemplate.class);
        when(redisTemplate.execute(ArgumentMatchers.<RedisScript<Long>>any(), anyList(), any()))
                .thenThrow(new RuntimeException("redis down"));
        ReflectionTestUtils.setField(limiter, "redisTemplate", redisTemplate);

        // Each request is counted exactly once by the local fallback, so a limit of 1
        // admits the first request and rejects the second.
        limiter.check("project-a", "hello");

        assertThatThrownBy(() -> limiter.check("project-a", "world"))
                .isInstanceOf(EdgeFunctionException.class)
                .extracting(ex -> ((EdgeFunctionException) ex).status())
                .isEqualTo(HttpStatus.TOO_MANY_REQUESTS);
    }
}
