package ai.nubase.deploy.controller;

import ai.nubase.deploy.service.AppWorkerDeployService;
import ai.nubase.deploy.service.AppWorkerService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import org.springframework.web.server.ResponseStatusException;

import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.Mockito.mock;

class AppWorkerPlatformControllerTest {

    private final AppWorkerDeployService deployService = mock(AppWorkerDeployService.class);
    private final AppWorkerService appWorkerService = mock(AppWorkerService.class);
    private final AppWorkerPlatformController controller = new AppWorkerPlatformController(
            deployService,
            appWorkerService,
            new ObjectMapper()
    );

    @Test
    void listRequiresExplicitProjectRefHeader() {
        assertThatThrownBy(() -> controller.listAppWorkers(null))
                .isInstanceOf(ResponseStatusException.class)
                .hasMessageContaining("x-nubase-project-ref is required");
    }
}
