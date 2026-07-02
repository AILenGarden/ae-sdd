package ai.nubase.postgrest;

import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.TestPropertySource;

@SpringBootTest
@TestPropertySource(properties = {
    "pgrst.db-uri=jdbc:postgresql://localhost:5432/postgres",
    "pgrst.db-schemas=public",
    "pgrst.db-anon-role=anon"
})
class PostgRESTApplicationTests {

    @Test
    void contextLoads() {
        // Application context should load successfully
    }
}
