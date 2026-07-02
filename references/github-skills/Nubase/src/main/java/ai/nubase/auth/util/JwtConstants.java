package ai.nubase.auth.util;

/**
 * JWT-related constants.
 * Defines the various constant values used during JWT token processing.
 */
public final class JwtConstants {

    private JwtConstants() {
    }

    // JWT claim field names
    public static final String CLAIM_EMAIL = "email";
    public static final String CLAIM_SUBJECT = "sub";
    public static final String CLAIM_ROLE = "role";
    public static final String CLAIM_URL = "url";
    public static final String CLAIM_OWNER = "owner";
    public static final String CLAIM_ISSUED_AT = "iat";
    public static final String CLAIM_EXPIRES_AT = "exp";
    public static final String CLAIM_ISSUER = "iss";
    public static final String CLAIM_AUDIENCE = "aud";

    // JWT algorithm constants
    public static final String ALGORITHM_HMAC256 = "HS256";
    public static final String ALGORITHM_RS256 = "RS256";

    // JWT default configuration
    public static final int DEFAULT_EXPIRATION_MINUTES = 3; // 3 minutes
    public static final int DEFAULT_EXPIRATION_HOURS = 1; // 1 hour (fallback)
    public static final int DEFAULT_EXPIRATION_DAYS = 1;

    // JWT error messages
    public static final String ERROR_INVALID_TOKEN = "Invalid JWT token";
    public static final String ERROR_EXPIRED_TOKEN = "JWT token has expired";
    public static final String ERROR_MISSING_CLAIM = "JWT token is missing required claim: {}";
    public static final String ERROR_TOKEN_GENERATION_FAILED = "Failed to generate JWT token";
    public static final String ERROR_TOKEN_VALIDATION_FAILED = "JWT token validation failed";

    // Application-related error messages
    public static final String ERROR_APP_NOT_EXIST = "Application does not exist, appCode: {}";
    public static final String ERROR_APP_SETTING_EMPTY = "Application configuration is empty, appCode: {}";
    public static final String ERROR_JWT_SECRET_EMPTY = "Application JWT secret is empty, appCode: {}";
    public static final String ERROR_JWT_SECRET_FETCH_FAILED = "Exception occurred while fetching application JWT secret, appCode: {}";

    // JWT log message templates
    public static final String LOG_TOKEN_GENERATED = "Successfully generated JWT token, user: {}";
    public static final String LOG_TOKEN_VALIDATED = "JWT token validated successfully, user: {}";
    public static final String LOG_TOKEN_EXPIRED = "JWT token has expired, user: {}";
    public static final String LOG_TOKEN_INVALID = "JWT token is invalid, user: {}";
    public static final String LOG_JWT_SECRET_FETCHED = "Successfully fetched application JWT secret, appCode: {}";
}
