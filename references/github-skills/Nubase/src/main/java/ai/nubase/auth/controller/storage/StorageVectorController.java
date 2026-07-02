package ai.nubase.auth.controller.storage;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.auth.service.StorageVectorService;
import lombok.RequiredArgsConstructor;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;

@RestController
@RequiredArgsConstructor
@RequestMapping("/storage/v1/vector")
@ConditionalOnProperty(name = "nubase.storage.s3vectors.enabled", havingValue = "true")
@RequireServiceRole
public class StorageVectorController {

    private final StorageVectorService storageVectorService;

    @PostMapping("/CreateVectorBucket")
    public ResponseEntity<Void> createVectorBucket(@RequestBody Map<String, Object> body) {
        storageVectorService.createVectorBucket(asString(body.get("vectorBucketName")));
        return ResponseEntity.ok().build();
    }

    @PostMapping("/DeleteVectorBucket")
    public ResponseEntity<Void> deleteVectorBucket(@RequestBody Map<String, Object> body) {
        storageVectorService.deleteVectorBucket(asString(body.get("vectorBucketName")));
        return ResponseEntity.ok().build();
    }

    @PostMapping("/ListVectorBuckets")
    public ResponseEntity<Map<String, Object>> listVectorBuckets(@RequestBody(required = false) Map<String, Object> body) {
        Map<String, Object> request = body == null ? Map.of() : body;
        return ResponseEntity.ok(storageVectorService.listVectorBuckets(
                asString(request.get("prefix")),
                asInt(request.get("maxResults")),
                asString(request.get("nextToken"))
        ));
    }

    @PostMapping("/GetVectorBucket")
    public ResponseEntity<Map<String, Object>> getVectorBucket(@RequestBody Map<String, Object> body) {
        return ResponseEntity.ok(storageVectorService.getVectorBucket(asString(body.get("vectorBucketName"))));
    }

    @PostMapping("/CreateIndex")
    public ResponseEntity<Void> createIndex(@RequestBody Map<String, Object> body) {
        storageVectorService.createIndex(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName")),
                asInt(body.get("dimension")),
                asString(body.get("distanceMetric")),
                asString(body.get("dataType")),
                castMap(body.get("metadataConfiguration"))
        );
        return ResponseEntity.ok().build();
    }

    @PostMapping("/DeleteIndex")
    public ResponseEntity<Void> deleteIndex(@RequestBody Map<String, Object> body) {
        storageVectorService.deleteIndex(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName"))
        );
        return ResponseEntity.ok().build();
    }

    @PostMapping("/ListIndexes")
    public ResponseEntity<Map<String, Object>> listIndexes(@RequestBody Map<String, Object> body) {
        return ResponseEntity.ok(storageVectorService.listIndexes(
                asString(body.get("vectorBucketName")),
                asString(body.get("prefix")),
                asInt(body.get("maxResults")),
                asString(body.get("nextToken"))
        ));
    }

    @PostMapping("/GetIndex")
    public ResponseEntity<Map<String, Object>> getIndex(@RequestBody Map<String, Object> body) {
        return ResponseEntity.ok(storageVectorService.getIndex(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName"))
        ));
    }

    @PostMapping("/PutVectors")
    public ResponseEntity<Map<String, Object>> putVectors(@RequestBody Map<String, Object> body) {
        return ResponseEntity.ok(storageVectorService.putVectors(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName")),
                castListMap(body.get("vectors"))
        ));
    }

    @PostMapping("/GetVectors")
    public ResponseEntity<Map<String, Object>> getVectors(@RequestBody Map<String, Object> body) {
        return ResponseEntity.ok(storageVectorService.getVectors(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName")),
                castStringList(body.get("keys")),
                Boolean.TRUE.equals(body.get("returnData")),
                Boolean.TRUE.equals(body.get("returnMetadata"))
        ));
    }

    @PostMapping("/ListVectors")
    public ResponseEntity<Map<String, Object>> listVectors(@RequestBody Map<String, Object> body) {
        return ResponseEntity.ok(storageVectorService.listVectors(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName")),
                asInt(body.get("maxResults")),
                Boolean.TRUE.equals(body.get("returnData")),
                Boolean.TRUE.equals(body.get("returnMetadata")),
                asString(body.get("nextToken"))
        ));
    }

    @PostMapping("/QueryVectors")
    public ResponseEntity<Map<String, Object>> queryVectors(@RequestBody Map<String, Object> body) {
        Map<String, Object> queryVector = castMap(body.get("queryVector"));
        return ResponseEntity.ok(storageVectorService.queryVectors(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName")),
                queryVector == null ? List.of() : castDoubleList(queryVector.get("float32")),
                asInt(body.get("topK")),
                Boolean.TRUE.equals(body.get("returnDistance")),
                Boolean.TRUE.equals(body.get("returnMetadata")),
                castMap(body.get("filter"))
        ));
    }

    @PostMapping("/DeleteVectors")
    public ResponseEntity<Void> deleteVectors(@RequestBody Map<String, Object> body) {
        storageVectorService.deleteVectors(
                asString(body.get("vectorBucketName")),
                asString(body.get("indexName")),
                castStringList(body.get("keys"))
        );
        return ResponseEntity.ok().build();
    }

    private String asString(Object value) {
        return value == null ? null : value.toString();
    }

    private Integer asInt(Object value) {
        if (value == null) {
            return null;
        }
        if (value instanceof Number number) {
            return number.intValue();
        }
        return Integer.parseInt(value.toString());
    }

    @SuppressWarnings("unchecked")
    private Map<String, Object> castMap(Object value) {
        if (!(value instanceof Map<?, ?>)) {
            return null;
        }
        return (Map<String, Object>) value;
    }

    @SuppressWarnings("unchecked")
    private List<Map<String, Object>> castListMap(Object value) {
        if (!(value instanceof List<?> list)) {
            return List.of();
        }
        return (List<Map<String, Object>>) list;
    }

    private List<String> castStringList(Object value) {
        if (!(value instanceof List<?> list)) {
            return List.of();
        }
        return list.stream().map(item -> item == null ? null : item.toString()).toList();
    }

    private List<Double> castDoubleList(Object value) {
        if (!(value instanceof List<?> list)) {
            return List.of();
        }
        return list.stream().map(item -> {
            if (item instanceof Number number) {
                return number.doubleValue();
            }
            return Double.parseDouble(item.toString());
        }).toList();
    }
}
