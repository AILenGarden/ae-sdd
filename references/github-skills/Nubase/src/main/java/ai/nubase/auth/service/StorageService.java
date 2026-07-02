package ai.nubase.auth.service;

import ai.nubase.auth.converter.StorageConverter;
import ai.nubase.auth.dto.storage.DownloadResult;
import ai.nubase.auth.dto.storage.FileListRequest;
import ai.nubase.auth.dto.storage.ObjectCopyRequest;
import ai.nubase.auth.dto.storage.ObjectDTO;
import ai.nubase.auth.dto.storage.ObjectMoveRequest;
import ai.nubase.auth.dto.storage.SignedUploadToken;
import ai.nubase.auth.dto.storage.SignedUploadUrlRequest;
import ai.nubase.auth.dto.storage.SignedUrlResponse;
import ai.nubase.auth.dto.storage.UploadRequest;
import ai.nubase.auth.dto.storage.UploadResponse;
import ai.nubase.auth.entity.Bucket;
import ai.nubase.auth.util.BucketUtil;
import ai.nubase.auth.util.JwtConstants;
import ai.nubase.auth.util.JwtUtil;
import ai.nubase.auth.util.StorageKeyResolver;
import ai.nubase.auth.entity.StorageObject;
import ai.nubase.auth.repository.StorageObjectRepository;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.platform.storage.R2ClientProvider;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.jsonwebtoken.Claims;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.MediaType;
import org.springframework.core.io.InputStreamResource;
import org.springframework.core.io.Resource;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.util.CollectionUtils;

import software.amazon.awssdk.core.sync.RequestBody;
import software.amazon.awssdk.services.s3.model.CopyObjectRequest;
import software.amazon.awssdk.services.s3.model.DeleteObjectRequest;
import software.amazon.awssdk.services.s3.model.GetObjectRequest;
import software.amazon.awssdk.services.s3.model.PutObjectRequest;
import software.amazon.awssdk.services.s3.model.PutObjectResponse;
import org.apache.commons.lang3.StringUtils;
import ai.nubase.common.constant.S3MetadataConstant;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.*;
import java.util.stream.Collectors;

import static ai.nubase.auth.util.JwtConstants.CLAIM_URL;

/**
 * Service for managing storage objects (files)
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class StorageService {

    private static final int DEFAULT_SIGNED_URL_EXPIRES_SECONDS = 3600;
    private static final int DEFAULT_UPLOAD_SIGN_EXPIRES_SECONDS = 7200;

    private final StorageObjectRepository storageObjectRepository;
    private final BucketService bucketService;
    private final R2ClientProvider r2;
    private final ObjectMapper objectMapper;
    private final StorageConverter storageConverter;


    @Transactional
    public UploadResponse doUpload(UploadRequest req, boolean isUpsert) {
        String bucketId = req.getBucketId();
        String path = req.getPath();
        UUID owner = req.getOwnerOverride();

        log.info("Uploading file: {}/{}", bucketId, path);

        Bucket bucket = bucketService.getBucketOrThrow(bucketId);

        if (Objects.isNull(owner) && !MultiTenancyContext.isServiceRole()) {
            throw new IllegalArgumentException("Forbidden");
        }

        StorageObject existingObject = storageObjectRepository.findByBucketIdAndName(bucketId, path)
                .orElse(null);
        if (!isUpsert && existingObject != null) {
            throw new IllegalArgumentException("The resource already exists");
        }

        byte[] bytes = req.getFileBytes();
        if (bytes == null || bytes.length == 0) {
            throw new IllegalArgumentException("No content provided");
        }

        String originalFilename = path.contains("/") ? path.substring(path.lastIndexOf('/') + 1) : path;

        long fileSize = req.getFileSize();
        String effectiveContentType = StringUtils.isBlank(req.getContentType())
                ? MediaType.APPLICATION_OCTET_STREAM_VALUE : req.getContentType();
        RequestBody requestBody = RequestBody.fromBytes(bytes);

        BucketUtil.validateFileUpload(bucket, fileSize, effectiveContentType);

        // Parse cacheControl: pure digits become max-age=N, blank defaults to no-cache
        String rawCacheControl = req.getCacheControl();
        String effectiveCacheControl;
        if (StringUtils.isBlank(rawCacheControl)) {
            effectiveCacheControl = "no-cache";
        } else if (rawCacheControl.matches("\\d+")) {
            effectiveCacheControl = "max-age=" + rawCacheControl;
        } else {
            effectiveCacheControl = rawCacheControl;
        }

        Map<String, Object> userMetadata = null;
        if (StringUtils.isNotBlank(req.getXMetadata())) {
            try {
                String json = new String(Base64.getDecoder().decode(req.getXMetadata()), StandardCharsets.UTF_8);
                userMetadata = objectMapper.readValue(json, new TypeReference<>() {
                });
            } catch (Exception ignored) {
            }
        }

        Map<String, String> s3Metadata = new HashMap<>();
        if (StringUtils.isNotBlank(originalFilename)) {
            s3Metadata.put(S3MetadataConstant.ORIGINAL_FILENAME, originalFilename);
        }
        s3Metadata.put(S3MetadataConstant.CONTENT_TYPE, effectiveContentType);
        s3Metadata.put(S3MetadataConstant.SIZE, String.valueOf(fileSize));

        // Save database metadata first
        StorageObject storageObject = Objects.nonNull(existingObject) ? existingObject : new StorageObject();
        storageObject.setBucketId(bucketId);
        storageObject.setName(path);
        storageObject.setOwner(owner);
        storageObject.setVersion(UUID.randomUUID().toString());

        Map<String, Object> metadata = new HashMap<>();
        metadata.put(S3MetadataConstant.SIZE, fileSize);
        metadata.put(S3MetadataConstant.MIMETYPE, effectiveContentType);
        metadata.put(S3MetadataConstant.CACHE_CONTROL, effectiveCacheControl);
        metadata.put(S3MetadataConstant.LAST_MODIFIED, Instant.now().toString());
        storageObject.setMetadata(metadata);
        storageObject.setUserMetadata(userMetadata);

        storageObject = storageObjectRepository.save(storageObject);

        // Then upload to object storage (using the global shared bucket with a tenant prefix)
        String s3Key = StorageKeyResolver.resolveKey(bucket.getName(), path);
        PutObjectRequest putRequest = PutObjectRequest.builder()
                .bucket(r2.bucket()).key(s3Key)
                .contentType(effectiveContentType).contentLength(fileSize)
                .cacheControl(effectiveCacheControl).metadata(s3Metadata).build();

        PutObjectResponse putResponse = r2.s3().putObject(putRequest, requestBody);
        log.info("File uploaded to R2: s3Key={}, eTag={}", s3Key, putResponse.eTag());
        if (isUpsert && existingObject != null) {
            log.info("Upsert overwrote existing object: {}/{}", bucketId, path);
        }

        // Backfill eTag
        metadata.put(S3MetadataConstant.ETAG, putResponse.eTag());
        storageObject.setMetadata(metadata);
        storageObjectRepository.save(storageObject);

        return UploadResponse.builder()
                .id(storageObject.getId().toString())
                .key(bucketId + "/" + path)
                .build();
    }

    /**
     * Download a file from a bucket and return both the resource and its metadata.
     */
    public DownloadResult downloadFile(String bucketId, String path) {
        log.info("Downloading file: {}/{}", bucketId, path);

        Bucket bucket = bucketService.getBucketOrThrow(bucketId);

        StorageObject storageObject = storageObjectRepository.findByBucketIdAndName(bucketId, path)
                .orElseThrow(() -> new IllegalArgumentException("Object not found: " + path));

        String s3Key = StorageKeyResolver.resolveKey(bucket.getName(), path);
        log.info("Downloading from R2: s3Key={}", s3Key);
        GetObjectRequest getRequest = GetObjectRequest.builder()
                .bucket(r2.bucket())
                .key(s3Key)
                .build();
        InputStream inputStream = r2.s3().getObject(getRequest);

        storageObject.setLastAccessedAt(Instant.now());
        storageObjectRepository.save(storageObject);
        ObjectDTO metadata = storageConverter.toDTO(storageObject);

        log.info("Download completed: {}/{}", bucketId, path);
        Resource resource = new InputStreamResource(inputStream);
        return new DownloadResult(resource, metadata);

    }

    /**
     * Delete a single file from a bucket.
     */
    public void deleteFile(String bucketId, String path) {
        log.info("Deleting file: {}/{}", bucketId, path);

        Bucket bucket = bucketService.getBucketOrThrow(bucketId);

        StorageObject storageObject = storageObjectRepository.findByBucketIdAndName(bucketId, path)
                .orElseThrow(() -> new IllegalArgumentException("Object not found: " + path));

        // Delete database record first
        storageObjectRepository.delete(storageObject);
        log.info("Database record deleted: {}/{}", bucketId, path);

        // Then delete from object storage
        String s3Key = StorageKeyResolver.resolveKey(bucket.getName(), path);
        log.info("Deleting from R2: s3Key={}", s3Key);
        DeleteObjectRequest deleteRequest = DeleteObjectRequest.builder()
                .bucket(r2.bucket())
                .key(s3Key)
                .build();
        r2.s3().deleteObject(deleteRequest);
        log.info("R2 object deleted: s3Key={}", s3Key);
    }

    /**
     * Batch delete files.
     */
    public List<ObjectDTO> deleteFiles(String bucketId, List<String> prefixes) {
        log.info("Batch deleting files: bucketId={}, count={}", bucketId, prefixes == null ? 0 : prefixes.size());
        if (CollectionUtils.isEmpty(prefixes)) {
            throw new IllegalArgumentException("prefixes is required");
        }

        List<ObjectDTO> deleted = new ArrayList<>();
        for (String prefix : prefixes) {
            StorageObject existing = storageObjectRepository.findByBucketIdAndName(bucketId, prefix).
                    orElse(null);
            if (existing == null) {
                continue;
            }
            deleted.add(storageConverter.toDTO(existing));
            deleteFile(bucketId, prefix);
        }
        log.info("Batch delete completed: bucketId={}, deletedCount={}", bucketId, deleted.size());
        return deleted;
    }

    /**
     * List files in a bucket with optional filtering.
     */
    public List<ObjectDTO> listFiles(FileListRequest request) {
        String bucketId = request.getBucketId();
        String prefix = request.getPrefix();
        Integer limit = request.getLimit();
        Integer offset = request.getOffset();

        log.info("Listing files: bucketId={}, prefix={}, limit={}, offset={}", bucketId, prefix, limit, offset);
        bucketService.getBucketOrThrow(bucketId);

        List<StorageObject> objects = StringUtils.isNotBlank(prefix)
                ? storageObjectRepository.findByBucketIdAndNamePrefix(bucketId, prefix)
                : storageObjectRepository.findByBucketId(bucketId);

        String keyword = StringUtils.trimToNull(request.getSearch());
        String orderBy = StringUtils.defaultIfBlank(request.getSortColumn(), "name");
        boolean descending = "desc".equalsIgnoreCase(request.getSortOrder());
        int fromIndex = Math.max(offset == null ? 0 : offset, 0);

        if (keyword != null) {
            objects = objects.stream()
                    .filter(object -> StringUtils.containsIgnoreCase(object.getName(), keyword))
                    .collect(Collectors.toList());
        }

        Comparator<StorageObject> comparator = switch (orderBy) {
            case "updated_at" ->
                    Comparator.comparing(StorageObject::getUpdatedAt, Comparator.nullsLast(Instant::compareTo));
            case "created_at" ->
                    Comparator.comparing(StorageObject::getCreatedAt, Comparator.nullsLast(Instant::compareTo));
            case "last_accessed_at" ->
                    Comparator.comparing(StorageObject::getLastAccessedAt, Comparator.nullsLast(Instant::compareTo));
            default -> Comparator.comparing(StorageObject::getName, Comparator.nullsLast(String::compareTo));
        };

        objects.sort(descending ? comparator.reversed() : comparator);

        if (fromIndex >= objects.size()) {
            return List.of();
        }

        int toIndex = objects.size();
        if (limit != null && limit > 0) {
            toIndex = Math.min(fromIndex + limit, objects.size());
        }

        List<ObjectDTO> result = objects.subList(fromIndex, toIndex).stream()
                .map(storageConverter::toDTO)
                .collect(Collectors.toList());
        log.info("Listed files: bucketId={}, resultCount={}", bucketId, result.size());
        return result;
    }


    /**
     * Create a signed URL for object access.
     */
    public SignedUrlResponse createSignedUrl(String bucketId, String path, Integer expiresIn) {
        log.info("Creating signed URL: bucketId={}, path={}, expiresIn={}", bucketId, path, expiresIn);
        storageObjectRepository.findByBucketIdAndName(bucketId, path)
                .orElseThrow(() -> new IllegalArgumentException("Object not found: " + path));

        int ttl = expiresIn != null && expiresIn > 0 ? expiresIn : DEFAULT_SIGNED_URL_EXPIRES_SECONDS;
        Map<String, Object> claims = Map.of(CLAIM_URL, bucketId + "/" + path);
        String token = JwtUtil.createToken(MultiTenancyContext.getJwtSecretOrThrow(), claims, ttl);

        return SignedUrlResponse.builder()
                .signedUrl("/object/sign/" + bucketId + "/" + path + "?token=" + token)
                .build();
    }

    /**
     * Create signed URLs for multiple objects.
     */
    public List<Map<String, Object>> createSignedUrls(String bucketId, List<String> paths, Integer expiresIn) {
        if (paths == null || paths.isEmpty()) {
            throw new IllegalArgumentException("paths is required");
        }

        List<Map<String, Object>> result = new ArrayList<>();
        for (String path : paths) {
            Map<String, Object> item = new HashMap<>();
            item.put("path", path);
            try {
                SignedUrlResponse signedUrl = createSignedUrl(bucketId, path, expiresIn);
                item.put("signedURL", signedUrl.getSignedUrl());
                item.put("error", null);
            } catch (Exception e) {
                item.put("signedURL", null);
                item.put("error", "Either the object does not exist or you do not have access to it");
            }
            result.add(item);
        }
        return result;
    }

    /**
     * Create a signed upload URL.
     */
    public Map<String, String> createSignedUploadUrl(SignedUploadUrlRequest request) {
        String bucketId = request.getBucketId();
        String path = request.getPath();
        log.info("Creating signed upload URL: bucketId={}, path={}", bucketId, path);

        Map<String, Object> claims = new HashMap<>();
        claims.put(CLAIM_URL, bucketId + "/" + path);
        claims.put("upsert", request.isUpsert());
        if (request.getOwner() != null) {
            claims.put(JwtConstants.CLAIM_OWNER, request.getOwner().toString());
        }

        String token = JwtUtil.createToken(MultiTenancyContext.getJwtSecretOrThrow(),
                claims, DEFAULT_UPLOAD_SIGN_EXPIRES_SECONDS);
        String url = "/object/upload/sign/" + bucketId + "/" + path + "?token=" + token;

        return Map.of("url", url, "token", token);
    }

    /**
     * Verify a signed token used for object download.
     */
    public void verifySignedObjectToken(String token, String bucketId, String path) {
        verifyUrlClaim(token, bucketId, path);
    }

    /**
     * Verify a signed token used for upload.
     * When bucketId and path are null, only the JWT signature and expiration are validated
     * (used for follow-up TUS requests).
     */
    public SignedUploadToken verifySignedUploadToken(String token, String bucketId, String path) {
        Claims claims = StringUtils.isNoneBlank(bucketId, path)
                ? verifyUrlClaim(token, bucketId, path)
                : JwtUtil.verifyToken(token, MultiTenancyContext.getJwtSecretOrThrow());
        boolean upsert = Boolean.parseBoolean(String.valueOf(claims.getOrDefault("upsert", "false")));
        UUID owner = null;
        Object ownerClaim = claims.get(JwtConstants.CLAIM_OWNER);
        if (ownerClaim instanceof String ownerString && !ownerString.isBlank()) {
            try {
                owner = UUID.fromString(ownerString);
            } catch (Exception ignored) {
                // Keep owner as null
            }
        }
        return new SignedUploadToken(owner, upsert);
    }

    /**
     * Copy an object.
     */
    public UploadResponse copyObject(ObjectCopyRequest request) {
        String bucketId = request.getBucketId();
        String sourceKey = request.getSourceKey();
        String destinationBucket = request.getDestinationBucket();
        String destinationKey = request.getDestinationKey();
        String targetBucketId = StringUtils.isBlank(destinationBucket) ? bucketId : destinationBucket;
        log.info("Copying object: source={}/{}, destination={}/{}", bucketId, sourceKey, targetBucketId, destinationKey);

        Bucket sourceBucket = bucketService.getBucketOrThrow(bucketId);
        Bucket targetBucket = bucketService.getBucketOrThrow(targetBucketId);

        StorageObject sourceObject = storageObjectRepository.findByBucketIdAndName(bucketId, sourceKey)
                .orElseThrow(() -> new IllegalArgumentException("Object not found: " + sourceKey));

        StorageObject existingTarget = storageObjectRepository.findByBucketIdAndName(targetBucketId, destinationKey)
                .orElse(null);
        if (!request.isUpsert() && existingTarget != null) {
            throw new IllegalArgumentException("The resource already exists");
        }

        // Save database record first
        StorageObject destinationObject = existingTarget != null ? existingTarget : new StorageObject();
        destinationObject.setBucketId(targetBucketId);
        destinationObject.setName(destinationKey);
        destinationObject.setOwner(request.getOwner() != null ? request.getOwner() : sourceObject.getOwner());
        destinationObject.setVersion(UUID.randomUUID().toString());

        Map<String, Object> metadata = sourceObject.getMetadata() != null
                ? new HashMap<>(sourceObject.getMetadata()) : new HashMap<>();
        if (request.getMetadataOverrides() != null) {
            metadata.putAll(request.getMetadataOverrides());
        }
        destinationObject.setMetadata(metadata);
        destinationObject.setUserMetadata(sourceObject.getUserMetadata());

        destinationObject = storageObjectRepository.save(destinationObject);

        // Then operate on object storage (using the global shared bucket with a tenant prefix)
        try {
            String sourceS3Key = StorageKeyResolver.resolveKey(sourceBucket.getName(), sourceKey);
            String destS3Key = StorageKeyResolver.resolveKey(targetBucket.getName(), destinationKey);
            CopyObjectRequest copyRequest = CopyObjectRequest.builder()
                    .sourceBucket(r2.bucket())
                    .sourceKey(sourceS3Key)
                    .destinationBucket(r2.bucket())
                    .destinationKey(destS3Key)
                    .build();
            r2.s3().copyObject(copyRequest);
            log.info("R2 copy completed: {} -> {}", sourceS3Key, destS3Key);
        } catch (Exception e) {
            throw new RuntimeException("Failed to copy file: " + e.getMessage(), e);
        }

        return UploadResponse.builder()
                .id(destinationObject.getId().toString())
                .key(targetBucketId + "/" + destinationKey)
                .build();
    }

    /**
     * Move an object.
     */
    public UploadResponse moveObject(ObjectMoveRequest request) {
        log.info("Moving object: source={}/{}, destination={}/{}",
                request.getBucketId(), request.getSourceKey(),
                request.getDestinationBucket(), request.getDestinationKey());
        UploadResponse copied = copyObject(ObjectCopyRequest.builder()
                .bucketId(request.getBucketId())
                .sourceKey(request.getSourceKey())
                .destinationBucket(request.getDestinationBucket())
                .destinationKey(request.getDestinationKey())
                .owner(request.getOwner())
                .upsert(true)
                .build());
        deleteFile(request.getBucketId(), request.getSourceKey());
        return copied;
    }

    /**
     * Get file metadata.
     */
    public ObjectDTO getObjectMetadata(String bucketId, String path) {
        log.info("Getting object metadata: {}/{}", bucketId, path);
        StorageObject storageObject = storageObjectRepository.findByBucketIdAndName(bucketId, path)
                .orElseThrow(() -> new IllegalArgumentException("Object not found: " + path));
        return storageConverter.toDTO(storageObject);
    }

    private Claims verifyUrlClaim(String token, String bucketId, String path) {
        Claims claims = JwtUtil.verifyToken(token, MultiTenancyContext.getJwtSecretOrThrow());

        Object urlClaim = claims.get(CLAIM_URL);
        String expected = bucketId + "/" + path;
        if (urlClaim == null || !expected.equals(urlClaim.toString())) {
            throw new IllegalArgumentException("Invalid signature");
        }

        return claims;
    }
}
