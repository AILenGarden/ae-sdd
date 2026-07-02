package ai.nubase.platform.event;

import org.springframework.context.ApplicationEvent;

/**
 * Fired after one or more keys in a settings category were written, so cached consumers
 * (e.g. the dynamic JavaMailSender) can rebuild themselves before the next use.
 */
public class SettingsChangedEvent extends ApplicationEvent {

    private final String category;

    public SettingsChangedEvent(Object source, String category) {
        super(source);
        this.category = category;
    }

    public String getCategory() {
        return category;
    }
}
