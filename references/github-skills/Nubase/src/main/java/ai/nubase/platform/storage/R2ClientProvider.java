package ai.nubase.platform.storage;

import ai.nubase.common.config.R2StorageProperties;
import ai.nubase.common.enums.R2RegionEnum;
import ai.nubase.platform.event.SettingsChangedEvent;
import ai.nubase.platform.service.PlatformSettingsService;
import jakarta.annotation.PostConstruct;
import jakarta.annotation.PreDestroy;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.context.event.EventListener;
import org.springframework.stereotype.Component;
import software.amazon.awssdk.auth.credentials.AwsBasicCredentials;
import software.amazon.awssdk.auth.credentials.StaticCredentialsProvider;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.S3Configuration;

import java.net.URI;
import java.util.Map;

/**
 * Runtime-rebuildable Cloudflare R2 / S3 client.
 *
 * <p>Reads endpoint, credentials, region, and bucket from
 * {@link PlatformSettingsService} category {@code storage_r2}, falling back to
 * {@link R2StorageProperties} (YAML / env) when a key is unset. The wrapped
 * {@link S3Client} is rebuilt on construction and whenever a {@link SettingsChangedEvent}
 * for {@code storage_r2} fires.
 *
 * <p>Callers should treat {@link #isConfigured()} as the guard: if it returns false,
 * storage features are not configured yet and should return 503 rather than NPE.
 */
@Slf4j
@Component
@RequiredArgsConstructor
public class R2ClientProvider {

    public static final String CATEGORY = "storage_r2";

    private final PlatformSettingsService settingsService;
    private final R2StorageProperties fallback;

    private volatile Snapshot snapshot;

    @PostConstruct
    void init() {
        rebuild();
    }

    @EventListener
    public void onSettingsChanged(SettingsChangedEvent event) {
        if (CATEGORY.equals(event.getCategory())) {
            log.info("R2 settings changed; rebuilding S3 client");
            rebuild();
        }
    }

    @PreDestroy
    void shutdown() {
        closeSilently(snapshot);
    }

    public S3Client s3() {
        Snapshot s = snapshot;
        if (s == null || s.client == null) {
            throw new IllegalStateException("R2 storage is not configured. Set it under "
                    + "Platform settings → Storage (R2) or via nubase.storage.r2.*");
        }
        return s.client;
    }

    public String bucket() {
        Snapshot s = snapshot;
        return s != null ? s.bucket : null;
    }

    public String publicUrl() {
        Snapshot s = snapshot;
        return s != null ? s.publicUrl : null;
    }

    public boolean isConfigured() {
        Snapshot s = snapshot;
        return s != null && s.client != null && s.bucket != null && !s.bucket.isEmpty();
    }

    private synchronized void rebuild() {
        Map<String, String> stored = settingsService.getCategory(CATEGORY);

        String endpoint    = firstNonBlank(stored.get("endpoint"),         fallback.getEndpoint());
        String region      = firstNonBlank(stored.get("region"),           fallback.getRegion());
        String accessKey   = firstNonBlank(stored.get("access_key_id"),    fallback.getAccessKeyId());
        String secretKey   = firstNonBlank(stored.get("secret_access_key"),fallback.getSecretAccessKey());
        String bucket      = firstNonBlank(stored.get("global_bucket"),    fallback.getGlobalBucket());
        String publicUrl   = firstNonBlank(stored.get("public_url"),       fallback.getPublicUrl());

        Snapshot prior = this.snapshot;

        if (isBlank(endpoint) || isBlank(accessKey) || isBlank(secretKey) || isBlank(bucket)) {
            log.warn("R2 storage not fully configured (missing endpoint / credentials / bucket); "
                    + "storage endpoints will return 503 until a super admin completes setup");
            this.snapshot = new Snapshot(null, bucket, publicUrl);
            closeSilently(prior);
            return;
        }

        R2RegionEnum r2Region = R2RegionEnum.fromValue(region)
                .orElseThrow(() -> new IllegalStateException("storage_r2.region is invalid: " + region));

        S3Configuration s3Config = S3Configuration.builder()
                .pathStyleAccessEnabled(true)
                .build();

        S3Client built = S3Client.builder()
                .endpointOverride(URI.create(endpoint))
                .credentialsProvider(StaticCredentialsProvider.create(
                        AwsBasicCredentials.create(accessKey, secretKey)))
                .region(Region.of(r2Region.getValue()))
                .serviceConfiguration(s3Config)
                .build();

        this.snapshot = new Snapshot(built, bucket, publicUrl);
        log.info("R2 client built: endpoint={} region={} bucket={} publicUrl={}",
                endpoint, region, bucket, publicUrl == null ? "<unset>" : publicUrl);

        closeSilently(prior);
    }

    private static void closeSilently(Snapshot s) {
        if (s != null && s.client != null) {
            try { s.client.close(); } catch (Exception ignored) { /* best effort */ }
        }
    }

    private static String firstNonBlank(String a, String b) {
        if (a != null && !a.isEmpty()) return a;
        return b;
    }

    private static boolean isBlank(String s) {
        return s == null || s.isEmpty();
    }

    private record Snapshot(S3Client client, String bucket, String publicUrl) { }
}
