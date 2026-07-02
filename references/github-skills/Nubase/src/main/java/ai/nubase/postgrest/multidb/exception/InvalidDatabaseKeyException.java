package ai.nubase.postgrest.multidb.exception;

/**
 * Exception thrown when database key format is invalid
 */
public class InvalidDatabaseKeyException extends RuntimeException {

    private final String databaseKey;

    public InvalidDatabaseKeyException(String databaseKey) {
        super("Invalid database key: " + databaseKey);
        this.databaseKey = databaseKey;
    }

    public InvalidDatabaseKeyException(String databaseKey, String message) {
        super(message);
        this.databaseKey = databaseKey;
    }

    public String getDatabaseKey() {
        return databaseKey;
    }
}
