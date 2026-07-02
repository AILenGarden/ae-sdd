package ai.nubase.auth.controller.storage;

import ai.nubase.auth.dto.storage.BucketDTO;
import ai.nubase.auth.dto.storage.CreateBucketRequest;
import ai.nubase.auth.dto.storage.UpdateBucketRequest;
import ai.nubase.auth.service.BucketService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import java.time.Instant;
import java.util.List;

import static ai.nubase.test.ControllerTestSupport.json;
import static ai.nubase.test.ControllerTestSupport.mockMvc;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.delete;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

class StorageBucketControllerTest {

    private BucketService bucketService;
    private MockMvc mvc;

    @BeforeEach
    void setUp() {
        bucketService = mock(BucketService.class);
        mvc = mockMvc(new StorageBucketController(bucketService));
    }

    @Test
    void listBucketsPassesPaginationAndSearchToService() throws Exception {
        when(bucketService.listBuckets(10, 20, "name", "asc", "avatar"))
                .thenReturn(List.of(bucket("avatars", true)));

        mvc.perform(get("/storage/v1/bucket")
                        .param("limit", "10")
                        .param("offset", "20")
                        .param("sortColumn", "name")
                        .param("sortOrder", "asc")
                        .param("search", "avatar"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].id").value("avatars"))
                .andExpect(jsonPath("$[0].public").value(true));
    }

    @Test
    void createBucketReturnsBucketName() throws Exception {
        when(bucketService.createBucket(any(CreateBucketRequest.class)))
                .thenReturn(bucket("avatars", false));

        mvc.perform(post("/storage/v1/bucket")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "name": "avatars",
                                  "id": "avatars",
                                  "public": false,
                                  "file_size_limit": 5242880,
                                  "allowed_mime_types": ["image/png", "image/jpeg"]
                                }
                                """))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("avatars"));
    }

    @Test
    void createBucketRejectsInvalidNameBeforeServiceCall() throws Exception {
        mvc.perform(post("/storage/v1/bucket")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "name": "bad_bucket_name",
                                  "public": false
                                }
                                """))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.error").value("validation_failed"))
                .andExpect(jsonPath("$.fields.name").exists());
    }

    @Test
    void getBucketReturnsOneBucket() throws Exception {
        when(bucketService.getBucket("avatars")).thenReturn(bucket("avatars", false));

        mvc.perform(get("/storage/v1/bucket/avatars"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value("avatars"))
                .andExpect(jsonPath("$.name").value("avatars"));
    }

    @Test
    void updateBucketReturnsSuccessMessage() throws Exception {
        UpdateBucketRequest request = UpdateBucketRequest.builder()
                .isPublic(true)
                .fileSizeLimit(1024L)
                .allowedMimeTypes(new String[]{"text/plain"})
                .build();

        mvc.perform(put("/storage/v1/bucket/docs")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(json(request)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.message").value("Successfully updated"));

        verify(bucketService).updateBucket(eq("docs"), any(UpdateBucketRequest.class));
    }

    @Test
    void deleteBucketReturnsSuccessMessage() throws Exception {
        mvc.perform(delete("/storage/v1/bucket/docs"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.message").value("Successfully deleted"));

        verify(bucketService).deleteBucket("docs");
    }

    @Test
    void emptyBucketReturnsQueuedMessage() throws Exception {
        mvc.perform(post("/storage/v1/bucket/docs/empty"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.message").value("Empty bucket has been queued. Completion may take up to an hour."));

        verify(bucketService).emptyBucket("docs");
    }

    private BucketDTO bucket(String id, boolean isPublic) {
        return BucketDTO.builder()
                .id(id)
                .name(id)
                .isPublic(isPublic)
                .type("STANDARD")
                .createdAt(Instant.parse("2026-05-24T00:00:00Z"))
                .build();
    }
}
