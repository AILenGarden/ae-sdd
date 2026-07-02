package ai.nubase.cron.dto;

import ai.nubase.metadata.cron.entity.ScheduledJob;
import ai.nubase.metadata.cron.entity.ScheduledJobRun;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

public final class ScheduledJobDtos {

    public static final String NAME_PATTERN = ai.nubase.common.util.IdentifierPatterns.RESOURCE_NAME;

    private ScheduledJobDtos() {
    }

    public record CreateScheduledJobRequest(
            @NotBlank @Pattern(regexp = NAME_PATTERN) String name,
            String description,
            @NotBlank @Size(max = 128) String cronExpression,
            @NotBlank String targetType,
            // edge_function target
            String functionSlug,
            String httpMethod,
            String requestPath,
            String requestBody,
            // db_function target
            String dbFunctionName,
            Map<String, Object> dbFunctionArgs,
            Integer timeoutSeconds,
            Boolean enabled
    ) {
    }

    public record UpdateScheduledJobRequest(
            String description,
            @Size(max = 128) String cronExpression,
            String functionSlug,
            String httpMethod,
            String requestPath,
            String requestBody,
            String dbFunctionName,
            Map<String, Object> dbFunctionArgs,
            Integer timeoutSeconds,
            Boolean enabled
    ) {
    }

    public record ScheduledJobResponse(
            UUID id,
            String projectRef,
            String name,
            String description,
            String cronExpression,
            String targetType,
            String functionSlug,
            String httpMethod,
            String requestPath,
            String requestBody,
            String dbFunctionName,
            String dbFunctionArgs,
            Integer timeoutSeconds,
            boolean enabled,
            Instant nextRunAt,
            Instant lastRunAt,
            String lastStatus,
            Instant createdAt,
            Instant updatedAt
    ) {
        public static ScheduledJobResponse from(ScheduledJob job) {
            return new ScheduledJobResponse(
                    job.getId(),
                    job.getProjectRef(),
                    job.getName(),
                    job.getDescription(),
                    job.getCronExpression(),
                    job.getTargetType(),
                    job.getFunctionSlug(),
                    job.getHttpMethod(),
                    job.getRequestPath(),
                    job.getRequestBody(),
                    job.getDbFunctionName(),
                    job.getDbFunctionArgs(),
                    job.getTimeoutSeconds(),
                    Boolean.TRUE.equals(job.getEnabled()),
                    job.getNextRunAt(),
                    job.getLastRunAt(),
                    job.getLastStatus(),
                    job.getCreatedAt(),
                    job.getUpdatedAt()
            );
        }
    }

    public record ScheduledJobRunResponse(
            UUID id,
            UUID jobId,
            String jobName,
            String targetType,
            Instant scheduledFor,
            Instant startedAt,
            Instant finishedAt,
            String status,
            String result,
            String errorMessage
    ) {
        public static ScheduledJobRunResponse from(ScheduledJobRun run) {
            return new ScheduledJobRunResponse(
                    run.getId(),
                    run.getJobId(),
                    run.getJobName(),
                    run.getTargetType(),
                    run.getScheduledFor(),
                    run.getStartedAt(),
                    run.getFinishedAt(),
                    run.getStatus(),
                    run.getResult(),
                    run.getErrorMessage()
            );
        }
    }
}
