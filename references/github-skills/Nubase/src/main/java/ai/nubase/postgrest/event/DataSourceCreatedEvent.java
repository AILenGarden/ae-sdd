package ai.nubase.postgrest.event;

import ai.nubase.postgrest.multidb.DatabaseConfig;
import lombok.Getter;
import org.springframework.context.ApplicationEvent;

/**
 * Event published when a new DataSource is created
 */
@Getter
public class DataSourceCreatedEvent extends ApplicationEvent {
    private final String databaseKey;
    private final DatabaseConfig config;

    public DataSourceCreatedEvent(Object source, String databaseKey, DatabaseConfig config) {
        super(source);
        this.databaseKey = databaseKey;
        this.config = config;
    }
}
