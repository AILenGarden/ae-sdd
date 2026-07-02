package ai.nubase.auth.service;

import ai.nubase.platform.storage.R2ClientProvider;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import software.amazon.awssdk.services.s3.model.HeadBucketRequest;

@Service
@RequiredArgsConstructor
public class StorageHealthService {

    private final R2ClientProvider r2;

    public void healthcheck() {
        if (!r2.isConfigured()) {
            throw new IllegalStateException("R2 storage is not configured");
        }
        HeadBucketRequest request = HeadBucketRequest.builder()
                .bucket(r2.bucket())
                .build();
        r2.s3().headBucket(request);
    }
}
