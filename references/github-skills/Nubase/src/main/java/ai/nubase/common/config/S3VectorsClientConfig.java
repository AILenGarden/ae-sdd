package ai.nubase.common.config;

import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import software.amazon.awssdk.auth.credentials.AwsBasicCredentials;
import software.amazon.awssdk.auth.credentials.StaticCredentialsProvider;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.s3vectors.S3VectorsClient;

import java.net.URI;

/**
 * AWS S3 Vectors Client configuration.
 * Mirrors the Supabase storage s3-vector.ts implementation.
 */
@Slf4j
@Configuration
@RequiredArgsConstructor
@ConditionalOnProperty(name = "nubase.storage.s3vectors.enabled", havingValue = "true")
public class S3VectorsClientConfig {

    private final S3VectorsProperties s3VectorsProperties;

    @Bean
    public S3VectorsClient s3VectorsClient() {
        log.info("Initializing S3 Vectors Client, region: {}, endpoint: {}",
                s3VectorsProperties.getRegion(),
                s3VectorsProperties.getEndpoint());

        AwsBasicCredentials credentials = AwsBasicCredentials.create(
                s3VectorsProperties.getAccessKeyId(),
                s3VectorsProperties.getSecretAccessKey()
        );

        var builder = S3VectorsClient.builder()
                .credentialsProvider(StaticCredentialsProvider.create(credentials))
                .region(Region.of(s3VectorsProperties.getRegion()));

        // Support custom endpoints (e.g. LocalStack)
        String endpoint = s3VectorsProperties.getEndpoint();
        if (endpoint != null && !endpoint.isBlank()) {
            builder.endpointOverride(URI.create(endpoint));
        }

        return builder.build();
    }
}
