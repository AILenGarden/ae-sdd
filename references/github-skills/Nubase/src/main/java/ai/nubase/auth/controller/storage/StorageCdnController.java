package ai.nubase.auth.controller.storage;

import ai.nubase.auth.service.StorageCdnService;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

@Slf4j
@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1/cdn")
public class StorageCdnController {

    private final StorageCdnService storageCdnService;

    @DeleteMapping("/{bucketId}/**")
    public ResponseEntity<Map<String, String>> purgeCache(@PathVariable String bucketId,
                                                          HttpServletRequest request
    ) {
        throw new UnsupportedOperationException("CDN cache purge is not supported yet");
    }
}
