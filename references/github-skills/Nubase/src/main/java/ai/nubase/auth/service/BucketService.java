package ai.nubase.auth.service;

import ai.nubase.auth.converter.StorageConverter;
import ai.nubase.auth.dto.storage.BucketDTO;
import ai.nubase.auth.dto.storage.CreateBucketRequest;
import ai.nubase.auth.dto.storage.UpdateBucketRequest;
import ai.nubase.auth.entity.Bucket;
import ai.nubase.auth.repository.BucketRepository;
import ai.nubase.auth.repository.StorageObjectRepository;
import ai.nubase.auth.util.StorageKeyResolver;
import ai.nubase.platform.storage.R2ClientProvider;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import software.amazon.awssdk.services.s3.model.*;

import java.util.*;
import java.util.stream.Collectors;

/**
 * Service for managing storage buckets
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class BucketService {

    private final BucketRepository bucketRepository;
    private final StorageObjectRepository storageObjectRepository;
    private final R2ClientProvider r2;
    private final StorageConverter storageConverter;

    /**
     * Create a new bucket
     */
    public BucketDTO createBucket(CreateBucketRequest request) {
        log.info("Creating bucket: {}", request.getName());

        if (bucketRepository.existsByName(request.getName())) {
            throw new IllegalArgumentException("Bucket already exists: " + request.getName());
        }

        Bucket bucket = Bucket.builder()
                .id(request.getName())
                .name(request.getName())
                .isPublic(request.getIsPublic())
                .avifAutodetection(request.getAvifAutodetection())
                .fileSizeLimit(request.getFileSizeLimit())
                .allowedMimeTypes(request.getAllowedMimeTypes())
                .build();

        bucket = bucketRepository.saveAndFlush(bucket);
        log.info("Bucket metadata saved: {} (global bucket: {}, prefix: {}/{}/) ",
                bucket.getName(), r2.bucket(),
                "appCode", bucket.getName());

        return storageConverter.toDTO(bucket);
    }

    /**
     * Get bucket by ID
     */
    public BucketDTO getBucket(String bucketId) {
        log.info("Getting bucket: {}", bucketId);
        Bucket bucket = bucketRepository.findById(bucketId)
                .orElseThrow(() -> new IllegalArgumentException("Bucket not found: " + bucketId));
        return storageConverter.toDTO(bucket);
    }

    /**
     * List all buckets
     */
    public List<BucketDTO> listBuckets(
            Integer limit,
            Integer offset,
            String sortColumn,
            String sortOrder,
            String search
    ) {
        log.info("Listing buckets: limit={}, offset={}, sortColumn={}, search={}", limit, offset, sortColumn, search);
        List<BucketDTO> buckets = bucketRepository.findAll().stream()
                .map(storageConverter::toDTO)
                .collect(Collectors.toList());

        String keyword = StringUtils.trimToNull(search);
        String orderBy = StringUtils.defaultIfBlank(sortColumn, "id");
        boolean descending = "desc".equalsIgnoreCase(sortOrder);
        int fromIndex = Math.max(offset == null ? 0 : offset, 0);

        if (keyword != null) {
            buckets = buckets.stream()
                    .filter(bucket -> StringUtils.containsIgnoreCase(bucket.getName(), keyword)
                            || StringUtils.containsIgnoreCase(bucket.getId(), keyword))
                    .collect(Collectors.toList());
        }

        Comparator<BucketDTO> comparator = switch (orderBy) {
            case "name" -> Comparator.comparing(BucketDTO::getName, Comparator.nullsLast(String::compareTo));
            case "created_at" ->
                    Comparator.comparing(BucketDTO::getCreatedAt, Comparator.nullsLast(java.time.Instant::compareTo));
            case "updated_at" ->
                    Comparator.comparing(BucketDTO::getUpdatedAt, Comparator.nullsLast(java.time.Instant::compareTo));
            default -> Comparator.comparing(BucketDTO::getId, Comparator.nullsLast(String::compareTo));
        };

        buckets.sort(descending ? comparator.reversed() : comparator);

        if (fromIndex >= buckets.size()) {
            return List.of();
        }

        int toIndex = buckets.size();
        if (limit != null && limit > 0) {
            toIndex = Math.min(fromIndex + limit, buckets.size());
        }

        List<BucketDTO> result = buckets.subList(fromIndex, toIndex);
        log.info("Listed buckets: resultCount={}", result.size());
        return result;
    }

    /**
     * Update bucket
     */
    public void updateBucket(String bucketId, UpdateBucketRequest request) {
        log.info("Updating bucket: {}", bucketId);

        Bucket bucket = bucketRepository.findById(bucketId)
                .orElseThrow(() -> new IllegalArgumentException("Bucket not found: " + bucketId));

        // Update fields
        Optional.ofNullable(request.getIsPublic()).ifPresent(bucket::setIsPublic);
        Optional.ofNullable(request.getAvifAutodetection()).ifPresent(bucket::setAvifAutodetection);
        Optional.ofNullable(request.getFileSizeLimit()).ifPresent(bucket::setFileSizeLimit);
        Optional.ofNullable(request.getAllowedMimeTypes()).ifPresent(bucket::setAllowedMimeTypes);

        bucket = bucketRepository.save(bucket);
        log.info("Bucket updated: {}", bucket.getName());
    }

    /**
     * Delete bucket
     */
    @Transactional
    public void deleteBucket(String bucketId) {
        log.info("Deleting bucket: {}", bucketId);

        Bucket bucket = getBucketOrThrow(bucketId);

        // Delete database records first
        storageObjectRepository.deleteByBucketId(bucketId);
        bucketRepository.delete(bucket);

        // Delete all objects of this logical bucket from the global bucket by prefix (no longer deleting the real S3 bucket)
        String keyPrefix = StorageKeyResolver.resolveBucketPrefix(bucket.getName());
        deleteR2ObjectsByPrefix(keyPrefix);
        log.info("Objects with prefix '{}' deleted from global bucket", keyPrefix);
    }

    /**
     * Empty bucket (delete all objects but keep the bucket)
     */
    @Transactional
    public void emptyBucket(String bucketId) {
        log.info("Emptying bucket: {}", bucketId);

        Bucket bucket = getBucketOrThrow(bucketId);

        // Delete database records first
        storageObjectRepository.deleteByBucketId(bucketId);

        // Delete all objects of this logical bucket from the global bucket by prefix
        String keyPrefix = StorageKeyResolver.resolveBucketPrefix(bucket.getName());
        deleteR2ObjectsByPrefix(keyPrefix);
        log.info("Bucket emptied (prefix: {}): {}", keyPrefix, bucket.getName());
    }

    /**
     * Batch delete objects in the global bucket by prefix
     */
    private void deleteR2ObjectsByPrefix(String keyPrefix) {
        String globalBucket = r2.bucket();
        ListObjectsV2Request listRequest = ListObjectsV2Request.builder()
                .bucket(globalBucket)
                .prefix(keyPrefix)
                .build();

        ListObjectsV2Response listResponse;
        do {
            listResponse = r2.s3().listObjectsV2(listRequest);
            List<ObjectIdentifier> keys = listResponse.contents().stream()
                    .map(s3Object -> ObjectIdentifier.builder().key(s3Object.key()).build())
                    .toList();

            if (!keys.isEmpty()) {
                log.info("Deleting R2 objects batch: prefix={}, batchSize={}", keyPrefix, keys.size());
                DeleteObjectsRequest deleteRequest = DeleteObjectsRequest.builder()
                        .bucket(globalBucket)
                        .delete(Delete.builder().objects(keys).quiet(true).build())
                        .build();
                r2.s3().deleteObjects(deleteRequest);
            }

            listRequest = listRequest.toBuilder()
                    .continuationToken(listResponse.nextContinuationToken())
                    .build();
        } while (listResponse.isTruncated());
    }

    public Bucket getBucketOrThrow(String bucketId) {
        return bucketRepository.findById(bucketId)
                .orElseThrow(() -> new IllegalArgumentException("Bucket not found: " + bucketId));
    }
}
