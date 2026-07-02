package ai.nubase.common.enums;

/**
 * Database initialization status
 */
public enum DatabaseInitStatus {
    /**
     * Configuration saved, waiting for physical database initialization
     */
    PENDING_INIT,

    /**
     * Physical database initialization in progress
     */
    INITIALIZING,

    /**
     * Physical database initialized successfully
     */
    INITIALIZED,

    /**
     * Physical database initialization failed
     */
    INIT_FAILED;

}
