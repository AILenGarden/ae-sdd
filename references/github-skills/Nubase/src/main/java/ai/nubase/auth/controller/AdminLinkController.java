package ai.nubase.auth.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.auth.dto.request.admin.GenerateLinkRequest;
import ai.nubase.auth.dto.response.admin.GenerateLinkResponse;
import ai.nubase.auth.service.AdminLinkService;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Admin generate-link API (service_role only): builds a verification action link without
 * sending an email. Mirrors Supabase GoTrue's {@code POST /admin/generate_link}.
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/admin/generate_link")
@RequireServiceRole
public class AdminLinkController {

    private final AdminLinkService adminLinkService;

    @PostMapping
    public ResponseEntity<GenerateLinkResponse> generate(@Valid @RequestBody GenerateLinkRequest request) {
        return ResponseEntity.ok(adminLinkService.generate(request));
    }
}
