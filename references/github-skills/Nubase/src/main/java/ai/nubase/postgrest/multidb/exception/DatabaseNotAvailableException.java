package ai.nubase.postgrest.multidb.exception;

/**
 * Exception thrown when a database exists but is not available
 * (e.g., unhealthy, connection failed)
 */
public class DatabaseNotAvailableException extends RuntimeException {

    private final String databaseKey;

    public DatabaseNotAvailableException(String databaseKey) {
        super("Database not available: " + databaseKey);
        this.databaseKey = databaseKey;
    }

    public DatabaseNotAvailableException(String databaseKey, String message) {
        super(message);
        this.databaseKey = databaseKey;
    }

    public DatabaseNotAvailableException(String databaseKey, Throwable cause) {
        super("Database not available: " + databaseKey, cause);
        this.databaseKey = databaseKey;
    }

    public String getDatabaseKey() {
        return databaseKey;
    }
}
