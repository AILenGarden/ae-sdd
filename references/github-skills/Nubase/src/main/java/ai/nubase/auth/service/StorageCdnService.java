package ai.nubase.auth.service;

import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

@Slf4j
@Service
@RequiredArgsConstructor
public class StorageCdnService {

    public void purgeCache(String bucketId, String path) {
        log.info("Purging cache for object {}/{} (best-effort no-op)", bucketId, path);
    }
}
