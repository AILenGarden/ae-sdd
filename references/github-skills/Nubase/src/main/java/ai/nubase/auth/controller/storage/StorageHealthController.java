package ai.nubase.auth.controller.storage;

import ai.nubase.auth.service.StorageHealthService;
import ai.nubase.common.enums.TrueOrFalseEnum;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1/health")
public class StorageHealthController {

    private final StorageHealthService storageHealthService;

    @GetMapping
    public ResponseEntity<Map<String, Object>> healthcheck() {
        try {
            storageHealthService.healthcheck();
            return ResponseEntity.ok(Map.of("healthy", TrueOrFalseEnum.TRUE.isBoolValue()));
        } catch (Exception e) {
            return ResponseEntity.ok(Map.of("healthy", TrueOrFalseEnum.FALSE.isBoolValue()));
        }
    }
}
