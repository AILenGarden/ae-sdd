package ai.nubase.postgrest.event;

import lombok.Getter;
import org.springframework.context.ApplicationEvent;

/**
 * Event published when a DataSource is removed
 */
@Getter
public class DataSourceRemovedEvent extends ApplicationEvent {
    private final String databaseKey;

    public DataSourceRemovedEvent(Object source, String databaseKey) {
        super(source);
        this.databaseKey = databaseKey;
    }
}
