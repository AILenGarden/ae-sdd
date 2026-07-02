package ai.nubase.auth.converter;

import ai.nubase.auth.dto.storage.BucketDTO;
import ai.nubase.auth.dto.storage.ObjectDTO;
import ai.nubase.auth.entity.Bucket;
import ai.nubase.auth.entity.StorageObject;
import ai.nubase.auth.enums.BucketType;
import org.springframework.stereotype.Component;

@Component
public class StorageConverter {

    public ObjectDTO toDTO(StorageObject object) {
        return ObjectDTO.builder()
                .id(object.getId())
                .name(object.getName())
                .bucketId(object.getBucketId())
                .owner(object.getOwner())
                .metadata(object.getMetadata())
                .createdAt(object.getCreatedAt())
                .updatedAt(object.getUpdatedAt())
                .lastAccessedAt(object.getLastAccessedAt())
                .build();
    }

    public BucketDTO toDTO(Bucket bucket) {
        return BucketDTO.builder()
                .id(bucket.getId())
                .type(BucketType.STANDARD.name())
                .name(bucket.getName())
                .owner(bucket.getOwner() != null ? bucket.getOwner().toString() : null)
                .isPublic(bucket.getIsPublic())
                .avifAutodetection(bucket.getAvifAutodetection())
                .fileSizeLimit(bucket.getFileSizeLimit())
                .allowedMimeTypes(bucket.getAllowedMimeTypes())
                .createdAt(bucket.getCreatedAt())
                .updatedAt(bucket.getUpdatedAt())
                .build();
    }
}
