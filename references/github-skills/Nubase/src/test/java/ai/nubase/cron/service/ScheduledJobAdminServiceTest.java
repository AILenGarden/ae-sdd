package ai.nubase.cron.service;

import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.cron.CronProperties;
import ai.nubase.cron.dto.ScheduledJobDtos.CreateScheduledJobRequest;
import ai.nubase.metadata.cron.entity.ScheduledJob;
import ai.nubase.metadata.cron.repository.ScheduledJobRepository;
import ai.nubase.metadata.cron.repository.ScheduledJobRunRepository;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.ArgumentCaptor;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.Map;

import static ai.nubase.cron.service.CronExceptions.CronException;
import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.lenient;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

@ExtendWith(MockitoExtension.class)
class ScheduledJobAdminServiceTest {

    @Mock
    private ScheduledJobRepository jobRepository;
    @Mock
    private ScheduledJobRunRepository runRepository;

    private ScheduledJobAdminService service;

    @BeforeEach
    void setUp() {
        service = new ScheduledJobAdminService(jobRepository, runRepository, new CronProperties(), new ObjectMapper());
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder().appCode("app1").build());
        lenient().when(jobRepository.existsByProjectRefAndName(anyString(), anyString())).thenReturn(false);
        lenient().when(jobRepository.save(any())).thenAnswer(inv -> inv.getArgument(0));
    }

    @AfterEach
    void tearDown() {
        MultiTenancyContext.clear();
    }

    @Test
    void createNormalizesCronAndComputesNextRun() {
        service.createJob(request("*/10 * * * *", ScheduledJob.TARGET_DB_FUNCTION, null, "refresh_stats"));

        ArgumentCaptor<ScheduledJob> captor = ArgumentCaptor.forClass(ScheduledJob.class);
        verify(jobRepository).save(captor.capture());
        ScheduledJob saved = captor.getValue();
        assertThat(saved.getCronExpression()).isEqualTo("0 */10 * * * *");
        assertThat(saved.getNextRunAt()).isNotNull();
        assertThat(saved.getProjectRef()).isEqualTo("app1");
    }

    @Test
    void createSerializesDbFunctionArgs() {
        service.createJob(new CreateScheduledJobRequest(
                "with-args", null, "0 3 * * *", ScheduledJob.TARGET_DB_FUNCTION,
                null, null, null, null,
                "refresh_stats", Map.of("days", 7), null, null));

        ArgumentCaptor<ScheduledJob> captor = ArgumentCaptor.forClass(ScheduledJob.class);
        verify(jobRepository).save(captor.capture());
        assertThat(captor.getValue().getDbFunctionArgs()).isEqualTo("{\"days\":7}");
    }

    @Test
    void createRejectsInvalidCron() {
        assertThatThrownBy(() -> service.createJob(request("bogus", ScheduledJob.TARGET_DB_FUNCTION, null, "fn")))
                .isInstanceOf(CronException.class)
                .satisfies(e -> assertThat(((CronException) e).code()).isEqualTo("INVALID_CRON"));
    }

    @Test
    void createRejectsMissingTargetFields() {
        assertThatThrownBy(() -> service.createJob(request("* * * * *", ScheduledJob.TARGET_EDGE_FUNCTION, null, null)))
                .isInstanceOf(CronException.class)
                .satisfies(e -> assertThat(((CronException) e).code()).isEqualTo("FUNCTION_SLUG_REQUIRED"));

        assertThatThrownBy(() -> service.createJob(request("* * * * *", ScheduledJob.TARGET_DB_FUNCTION, null, "1; DROP TABLE x")))
                .isInstanceOf(CronException.class)
                .satisfies(e -> assertThat(((CronException) e).code()).isEqualTo("INVALID_DB_FUNCTION"));

        assertThatThrownBy(() -> service.createJob(request("* * * * *", "webhook", null, null)))
                .isInstanceOf(CronException.class)
                .satisfies(e -> assertThat(((CronException) e).code()).isEqualTo("INVALID_TARGET_TYPE"));
    }

    @Test
    void createRejectsDuplicateNameAndBadTimeout() {
        when(jobRepository.existsByProjectRefAndName("app1", "dup")).thenReturn(true);
        assertThatThrownBy(() -> service.createJob(new CreateScheduledJobRequest(
                "dup", null, "* * * * *", ScheduledJob.TARGET_DB_FUNCTION,
                null, null, null, null, "fn", null, null, null)))
                .isInstanceOf(CronException.class)
                .satisfies(e -> assertThat(((CronException) e).code()).isEqualTo("JOB_EXISTS"));

        assertThatThrownBy(() -> service.createJob(new CreateScheduledJobRequest(
                "too-long", null, "* * * * *", ScheduledJob.TARGET_DB_FUNCTION,
                null, null, null, null, "fn", null, 100_000, null)))
                .isInstanceOf(CronException.class)
                .satisfies(e -> assertThat(((CronException) e).code()).isEqualTo("INVALID_TIMEOUT"));
    }

    private CreateScheduledJobRequest request(String cron, String targetType, String functionSlug, String dbFunction) {
        return new CreateScheduledJobRequest(
                "job-1", null, cron, targetType,
                functionSlug, null, null, null,
                dbFunction, null, null, null);
    }
}
