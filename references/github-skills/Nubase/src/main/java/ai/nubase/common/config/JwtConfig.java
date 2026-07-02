package ai.nubase.common.config;

import lombok.Getter;
import lombok.Setter;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.context.annotation.Configuration;

@Configuration
@ConfigurationProperties(prefix = "nubase.auth.jwt")
@Getter
@Setter
public class JwtConfig {
    private String secret;
    private String algorithm = "HS256";
    private int expiration = 3600; // 1 hour
    private String issuer = "nubase";
}
