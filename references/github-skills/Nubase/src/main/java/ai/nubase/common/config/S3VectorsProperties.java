package ai.nubase.common.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.context.annotation.Configuration;
import org.springframework.validation.annotation.Validated;

/**
 * AWS S3 Vectors configuration properties.
 * Vector storage uses the AWS S3 Vectors service, aligned with the Supabase Vector Bucket architecture.
 */
@Configuration
@ConfigurationProperties(prefix = "nubase.storage.s3vectors")
@Validated
@Data
public class S3VectorsProperties {

    /**
     * Whether to enable the vector storage feature; aligned with Supabase VECTOR_ENABLED.
     */
    private boolean enabled = false;

    /**
     * AWS region.
     */
    private String region = "us-east-1";

    /**
     * AWS access key ID.
     */
    private String accessKeyId;

    /**
     * AWS secret access key.
     */
    private String secretAccessKey;

    /**
     * Custom endpoint URL (leave empty to use the AWS default endpoint; may be a LocalStack address for local development).
     */
    private String endpoint;

    /**
     * S3 Vectors bucket name (used to actually store the vector data).
     */
    private String vectorBucketName = "nubase-vectors";

    /**
     * Maximum number of Vector Buckets per tenant.
     */
    private int maxBucketsPerTenant = 10;

    /**
     * Maximum number of indexes per bucket.
     */
    private int maxIndexesPerBucket = 10;
}
