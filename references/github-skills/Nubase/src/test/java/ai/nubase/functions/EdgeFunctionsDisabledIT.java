package ai.nubase.functions;

import ai.nubase.functions.controller.EdgeFunctionAdminController;
import ai.nubase.functions.controller.EdgeFunctionGatewayController;
import ai.nubase.functions.service.EdgeFunctionAdminService;
import ai.nubase.functions.service.EdgeFunctionInvocationRetentionService;
import ai.nubase.functions.service.EdgeFunctionInvocationService;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.ApplicationContext;
import org.springframework.test.context.ActiveProfiles;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Regression test for the NUBASE_FUNCTIONS_ENABLED kill switch: it used to be dead
 * config (nothing read it), leaving every /functions endpoint live. Conditional
 * wiring like this only fails at context-assembly time, so it needs a context test —
 * compilation and plain unit tests cannot catch it.
 */
@SpringBootTest(properties = "nubase.functions.enabled=false")
@ActiveProfiles("dev")
@DisplayName("Edge functions kill switch (dev metadata DB)")
class EdgeFunctionsDisabledIT {

    @Autowired
    private ApplicationContext context;

    @Test
    void functionsBeansAreAbsentWhenDisabled() {
        assertThat(context.getBeanNamesForType(EdgeFunctionGatewayController.class)).isEmpty();
        assertThat(context.getBeanNamesForType(EdgeFunctionAdminController.class)).isEmpty();
        assertThat(context.getBeanNamesForType(EdgeFunctionInvocationService.class)).isEmpty();
        assertThat(context.getBeanNamesForType(EdgeFunctionAdminService.class)).isEmpty();
        assertThat(context.getBeanNamesForType(EdgeFunctionInvocationRetentionService.class)).isEmpty();
    }
}
