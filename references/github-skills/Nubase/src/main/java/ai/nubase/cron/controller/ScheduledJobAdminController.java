package ai.nubase.cron.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.cron.dto.ScheduledJobDtos.CreateScheduledJobRequest;
import ai.nubase.cron.dto.ScheduledJobDtos.ScheduledJobResponse;
import ai.nubase.cron.dto.ScheduledJobDtos.ScheduledJobRunResponse;
import ai.nubase.cron.dto.ScheduledJobDtos.UpdateScheduledJobRequest;
import ai.nubase.cron.service.ScheduledJobAdminService;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;

@RestController
@RequestMapping("/cron/admin/v1")
@RequireServiceRole
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.cron.enabled", havingValue = "true", matchIfMissing = true)
public class ScheduledJobAdminController {

    private final ScheduledJobAdminService adminService;

    @GetMapping("/jobs")
    public ResponseEntity<List<ScheduledJobResponse>> list() {
        return ResponseEntity.ok(adminService.listJobs().stream().map(ScheduledJobResponse::from).toList());
    }

    @PostMapping("/jobs")
    public ResponseEntity<ScheduledJobResponse> create(@Valid @RequestBody CreateScheduledJobRequest request) {
        return ResponseEntity.ok(ScheduledJobResponse.from(adminService.createJob(request)));
    }

    @GetMapping("/jobs/{name}")
    public ResponseEntity<ScheduledJobResponse> get(@PathVariable String name) {
        return ResponseEntity.ok(ScheduledJobResponse.from(adminService.getJob(name)));
    }

    @PatchMapping("/jobs/{name}")
    public ResponseEntity<ScheduledJobResponse> update(
            @PathVariable String name,
            @Valid @RequestBody UpdateScheduledJobRequest request
    ) {
        return ResponseEntity.ok(ScheduledJobResponse.from(adminService.updateJob(name, request)));
    }

    @DeleteMapping("/jobs/{name}")
    public ResponseEntity<Map<String, Object>> delete(@PathVariable String name) {
        adminService.deleteJob(name);
        return ResponseEntity.ok(Map.of("ok", true));
    }

    @GetMapping("/jobs/{name}/runs")
    public ResponseEntity<List<ScheduledJobRunResponse>> jobRuns(
            @PathVariable String name,
            @RequestParam(defaultValue = "50") int limit
    ) {
        return ResponseEntity.ok(adminService.listRuns(name, limit).stream().map(ScheduledJobRunResponse::from).toList());
    }

    @GetMapping("/runs")
    public ResponseEntity<List<ScheduledJobRunResponse>> runs(@RequestParam(defaultValue = "50") int limit) {
        return ResponseEntity.ok(adminService.listRuns(null, limit).stream().map(ScheduledJobRunResponse::from).toList());
    }
}
