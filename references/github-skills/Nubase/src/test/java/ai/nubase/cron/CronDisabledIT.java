package ai.nubase.cron;

import ai.nubase.cron.controller.ScheduledJobAdminController;
import ai.nubase.cron.service.ScheduledJobAdminService;
import ai.nubase.cron.service.ScheduledJobRunner;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.ApplicationContext;
import org.springframework.test.context.ActiveProfiles;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * NUBASE_CRON_ENABLED=false must remove the whole module from the context —
 * conditional wiring only fails at context-assembly time, so it needs a context
 * test (same lesson as the functions kill switch).
 */
@SpringBootTest(properties = "nubase.cron.enabled=false")
@ActiveProfiles("dev")
@DisplayName("Cron kill switch (dev metadata DB)")
class CronDisabledIT {

    @Autowired
    private ApplicationContext context;

    @Test
    void cronBeansAreAbsentWhenDisabled() {
        assertThat(context.getBeanNamesForType(ScheduledJobRunner.class)).isEmpty();
        assertThat(context.getBeanNamesForType(ScheduledJobAdminController.class)).isEmpty();
        assertThat(context.getBeanNamesForType(ScheduledJobAdminService.class)).isEmpty();
    }
}
