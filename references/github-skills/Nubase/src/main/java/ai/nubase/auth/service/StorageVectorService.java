package ai.nubase.auth.service;

import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Service;

import java.util.List;
import java.util.Map;

@Service
@ConditionalOnProperty(name = "nubase.storage.s3vectors.enabled", havingValue = "true")
public class StorageVectorService {

    public void createVectorBucket(String vectorBucketName) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public void deleteVectorBucket(String vectorBucketName) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> listVectorBuckets(String prefix, Integer maxResults, String nextToken) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> getVectorBucket(String vectorBucketName) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public void createIndex(
            String vectorBucketName,
            String indexName,
            Integer dimension,
            String distanceMetric,
            String dataType,
            Map<String, Object> metadataConfiguration
    ) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public void deleteIndex(String vectorBucketName, String indexName) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> listIndexes(String vectorBucketName, String prefix, Integer maxResults, String nextToken) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> getIndex(String vectorBucketName, String indexName) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> putVectors(String vectorBucketName, String indexName, List<Map<String, Object>> vectors) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> getVectors(
            String vectorBucketName,
            String indexName,
            List<String> keys,
            boolean returnData,
            boolean returnMetadata
    ) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> listVectors(
            String vectorBucketName,
            String indexName,
            Integer maxResults,
            boolean returnData,
            boolean returnMetadata,
            String nextToken
    ) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public Map<String, Object> queryVectors(
            String vectorBucketName,
            String indexName,
            List<Double> query,
            Integer topK,
            boolean returnDistance,
            boolean returnMetadata,
            Map<String, Object> filter
    ) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }

    public void deleteVectors(String vectorBucketName, String indexName, List<String> keys) {
        throw new UnsupportedOperationException("Vector storage service is disabled");
    }
}
