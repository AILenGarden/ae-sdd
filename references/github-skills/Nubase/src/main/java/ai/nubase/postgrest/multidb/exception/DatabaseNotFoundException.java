package ai.nubase.postgrest.multidb.exception;

/**
 * Exception thrown when a requested database does not exist
 */
public class DatabaseNotFoundException extends RuntimeException {

    private final String databaseKey;

    public DatabaseNotFoundException(String databaseKey) {
        super("Database not found: " + databaseKey);
        this.databaseKey = databaseKey;
    }

    public DatabaseNotFoundException(String databaseKey, String message) {
        super(message);
        this.databaseKey = databaseKey;
    }

    public String getDatabaseKey() {
        return databaseKey;
    }
}
