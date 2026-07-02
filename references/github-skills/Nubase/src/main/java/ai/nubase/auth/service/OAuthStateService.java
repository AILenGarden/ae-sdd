package ai.nubase.auth.service;

import ai.nubase.auth.dto.oauth.OAuthStateData;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.time.Duration;

/**
 * OAuth State Management Service
 * Manages OAuth state data in Redis for security and context preservation
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class OAuthStateService {

    private static final String STATE_KEY_PREFIX = "oauth:state:";
    private static final int STATE_EXPIRATION_MINUTES = 10;

    private final StringRedisTemplate redisTemplate;
    private final ObjectMapper objectMapper = new ObjectMapper();

    /**
     * Store OAuth state data in Redis
     *
     * @param state    State token
     * @param stateData State context data
     */
    public void saveState(String state, OAuthStateData stateData) {
        try {
            String key = STATE_KEY_PREFIX + state;
            String jsonData = objectMapper.writeValueAsString(stateData);

            redisTemplate.opsForValue().set(
                    key,
                    jsonData,
                    Duration.ofMinutes(STATE_EXPIRATION_MINUTES)
            );

            log.debug("Saved OAuth state to Redis: state={}, provider={}, expires in {} minutes",
                    state, stateData.getProvider(), STATE_EXPIRATION_MINUTES);

        } catch (Exception e) {
            log.error("Failed to save OAuth state to Redis: {}", e.getMessage(), e);
            throw new RuntimeException("Failed to save OAuth state", e);
        }
    }

    /**
     * Retrieve and remove OAuth state data from Redis
     *
     * @param state State token
     * @return State context data, or null if not found or expired
     */
    public OAuthStateData getAndRemoveState(String state) {
        try {
            String key = STATE_KEY_PREFIX + state;
            String jsonData = redisTemplate.opsForValue().get(key);

            if (jsonData == null) {
                log.warn("OAuth state not found or expired: {}", state);
                return null;
            }

            // Delete state after retrieval (one-time use)
            redisTemplate.delete(key);

            OAuthStateData stateData = objectMapper.readValue(jsonData, OAuthStateData.class);
            log.debug("Retrieved and removed OAuth state from Redis: state={}, provider={}",
                    state, stateData.getProvider());

            return stateData;

        } catch (Exception e) {
            log.error("Failed to retrieve OAuth state from Redis: {}", e.getMessage(), e);
            return null;
        }
    }

    /**
     * Get OAuth state data without removing it (for JWT filter validation)
     *
     * @param state State token
     * @return State context data, or null if not found
     */
    public OAuthStateData getState(String state) {
        try {
            String key = STATE_KEY_PREFIX + state;
            String jsonData = redisTemplate.opsForValue().get(key);

            if (jsonData == null) {
                return null;
            }

            return objectMapper.readValue(jsonData, OAuthStateData.class);

        } catch (Exception e) {
            log.error("Failed to get OAuth state from Redis: {}", e.getMessage(), e);
            return null;
        }
    }
}
