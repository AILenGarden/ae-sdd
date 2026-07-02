package ai.nubase;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.autoconfigure.data.jpa.JpaRepositoriesAutoConfiguration;
import org.springframework.boot.autoconfigure.data.redis.RedisRepositoriesAutoConfiguration;
import org.springframework.boot.autoconfigure.security.servlet.SecurityAutoConfiguration;
import org.springframework.boot.context.properties.ConfigurationPropertiesScan;
import org.springframework.scheduling.annotation.EnableScheduling;
import org.springframework.transaction.annotation.EnableTransactionManagement;

/**
 * Supabase Auth (GoTrue) compatible API
 * Main application entry point
 * <p>
 * Excluded auto-configurations:
 * - SecurityAutoConfiguration: Uses a custom security configuration
 * - RedisRepositoriesAutoConfiguration: Redis is used manually only (via RedisTemplate)
 */
@SpringBootApplication(exclude = {
        SecurityAutoConfiguration.class,
        RedisRepositoriesAutoConfiguration.class,  // Disable Redis Repositories auto-scan
        JpaRepositoriesAutoConfiguration.class  // Use manually configured multi-datasource JPA Repository scanning
})
@ConfigurationPropertiesScan
@EnableScheduling
@EnableTransactionManagement
public class NuBaseApplication {
    public static void main(String[] args) {
        try {
            SpringApplication.run(NuBaseApplication.class, args);
        } catch (Throwable ex) {
            System.err.println("NuBase failed to start");
            ex.printStackTrace(System.err);
            throw ex;
        }
    }
}
