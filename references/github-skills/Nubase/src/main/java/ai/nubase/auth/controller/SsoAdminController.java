package ai.nubase.auth.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.auth.dto.request.sso.CreateSsoProviderRequest;
import ai.nubase.auth.dto.response.sso.SsoProviderResponse;
import ai.nubase.auth.service.SamlService;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

/**
 * Admin CRUD for SAML SSO providers (service_role only).
 * Base path: /auth/v1/admin/sso/providers — mirrors GoTrue's admin SSO API.
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/admin/sso/providers")
@RequireServiceRole
public class SsoAdminController {

    private final SamlService samlService;

    @PostMapping
    public ResponseEntity<SsoProviderResponse> create(@RequestBody CreateSsoProviderRequest request) {
        return ResponseEntity.ok(samlService.createProvider(request));
    }

    @GetMapping
    public ResponseEntity<List<SsoProviderResponse>> list() {
        return ResponseEntity.ok(samlService.listProviders());
    }

    @GetMapping("/{id}")
    public ResponseEntity<SsoProviderResponse> get(@PathVariable("id") String id) {
        return ResponseEntity.ok(samlService.getProvider(id));
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Map<String, String>> delete(@PathVariable("id") String id) {
        samlService.deleteProvider(id);
        return ResponseEntity.ok(Map.of("id", id));
    }
}
