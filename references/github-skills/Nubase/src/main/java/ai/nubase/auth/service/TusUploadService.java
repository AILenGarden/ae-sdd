package ai.nubase.auth.service;

import lombok.Data;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;
import software.amazon.awssdk.services.s3.model.CompletedPart;

import java.io.ByteArrayOutputStream;
import java.time.Instant;
import java.util.List;
import java.util.UUID;

@Service
public class TusUploadService {

    public UploadSession createSession(Long uploadLength, String uploadMetadata, String resourcePath,
                                       UUID owner, boolean upsert) {
        throw new UnsupportedOperationException("TUS upload service is disabled");
    }

    public UploadSession getSession(String id) {
        throw new UnsupportedOperationException("TUS upload service is disabled");
    }

    public UploadSession append(String id, byte[] chunk, Long offset) {
        throw new UnsupportedOperationException("TUS upload service is disabled");
    }

    public void deleteSession(String id) {
        throw new UnsupportedOperationException("TUS upload service is disabled");
    }

    @Scheduled(fixedRate = 3600000)
    public void cleanupStaleSessions() {
    }

    @Data
    public static class UploadSession {
        private String id;
        private Long uploadLength;
        private String uploadMetadata;
        private String resourcePath;
        private Long offset = 0L;
        private String s3UploadId;
        private String s3Key;
        private String bucketId;
        private String objectPath;
        private String contentType;
        private String cacheControl;
        private UUID owner;
        private boolean upsert;
        private ByteArrayOutputStream partBuffer;
        private List<CompletedPart> completedParts;
        private int nextPartNumber;
        private Instant createdAt;
    }
}
