package ai.nubase.auth.controller.storage;

import ai.nubase.auth.dto.storage.BucketDTO;
import ai.nubase.auth.dto.storage.CreateBucketRequest;
import ai.nubase.auth.dto.storage.UpdateBucketRequest;
import ai.nubase.auth.service.BucketService;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;

/**
 * Storage bucket controller.
 * Handles bucket CRUD operations.
 * Base path: /storage/v1/bucket
 */
@Slf4j
@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1/bucket")
public class StorageBucketController {

    private final BucketService bucketService;

    /**
     * Create a bucket.
     * POST /storage/v1/bucket
     */
    @PostMapping
    public ResponseEntity<Map<String, String>> createBucket(@Valid @RequestBody CreateBucketRequest request) {
        log.info("Creating bucket: {}", request.getName());
        BucketDTO bucket = bucketService.createBucket(request);
        return ResponseEntity.ok(Map.of("name", bucket.getName()));
    }

    /**
     * List all buckets.
     * GET /storage/v1/bucket
     */
    @GetMapping
    public ResponseEntity<List<BucketDTO>> listBuckets(
            @RequestParam(required = false) Integer limit,
            @RequestParam(required = false) Integer offset,
            @RequestParam(required = false) String sortColumn,
            @RequestParam(required = false) String sortOrder,
            @RequestParam(required = false) String search
    ) {
        List<BucketDTO> buckets = bucketService.listBuckets(limit, offset, sortColumn, sortOrder, search);
        return ResponseEntity.ok(buckets);
    }

    /**
     * Get a bucket by ID.
     * GET /storage/v1/bucket/{bucketId}
     */
    @GetMapping("/{bucketId}")
    public ResponseEntity<BucketDTO> getBucket(@PathVariable String bucketId) {
        BucketDTO bucket = bucketService.getBucket(bucketId);
        return ResponseEntity.ok(bucket);
    }

    /**
     * Update a bucket.
     * PUT /storage/v1/bucket/{bucketId}
     */
    @PutMapping("/{bucketId}")
    public ResponseEntity<Map<String, String>> updateBucket(
            @PathVariable String bucketId,
            @Valid @RequestBody UpdateBucketRequest request
    ) {
        bucketService.updateBucket(bucketId, request);
        return ResponseEntity.ok(Map.of("message", "Successfully updated"));
    }

    /**
     * Delete a bucket.
     * DELETE /storage/v1/bucket/{bucketId}
     */
    @DeleteMapping("/{bucketId}")
    public ResponseEntity<Map<String, String>> deleteBucket(@PathVariable String bucketId) {
        bucketService.deleteBucket(bucketId);
        return ResponseEntity.ok(Map.of("message", "Successfully deleted"));
    }

    /**
     * Empty a bucket (delete all objects).
     * POST /storage/v1/bucket/{bucketId}/empty
     */
    @PostMapping("/{bucketId}/empty")
    public ResponseEntity<Map<String, String>> emptyBucket(@PathVariable String bucketId) {
        bucketService.emptyBucket(bucketId);
        return ResponseEntity.ok(Map.of("message", "Empty bucket has been queued. Completion may take up to an hour."));
    }
}
