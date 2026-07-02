package ai.nubase.auth.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.auth.dto.response.mfa.FactorResponse;
import ai.nubase.auth.entity.MfaFactor;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.MfaFactorRepository;
import ai.nubase.auth.repository.SessionRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.service.TokenService;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Admin management of a user's sessions and MFA factors (service_role only).
 * Mirrors Supabase GoTrue's admin sessions/factors endpoints.
 *
 * <ul>
 *   <li>{@code GET    /auth/v1/admin/users/{id}/sessions}            — list active sessions</li>
 *   <li>{@code DELETE /auth/v1/admin/users/{id}/sessions}            — sign the user out everywhere</li>
 *   <li>{@code GET    /auth/v1/admin/users/{id}/factors}             — list enrolled MFA factors</li>
 *   <li>{@code DELETE /auth/v1/admin/users/{id}/factors/{factorId}}  — unenroll a factor</li>
 * </ul>
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/admin/users/{id}")
@RequireServiceRole
public class AdminUserSecurityController {

    private final UserRepository userRepository;
    private final SessionRepository sessionRepository;
    private final MfaFactorRepository mfaFactorRepository;
    private final TokenService tokenService;

    @GetMapping("/sessions")
    public ResponseEntity<List<Map<String, Object>>> listSessions(@PathVariable("id") UUID userId) {
        requireUser(userId);
        List<Map<String, Object>> sessions = sessionRepository.findByUserId(userId).stream()
                .map(this::toSessionMap)
                .toList();
        return ResponseEntity.ok(sessions);
    }

    @DeleteMapping("/sessions")
    @Transactional
    public ResponseEntity<Map<String, Object>> revokeSessions(@PathVariable("id") UUID userId) {
        User user = requireUser(userId);
        tokenService.revokeAllUserTokens(user);   // revoke refresh tokens
        sessionRepository.deleteByUserId(userId);  // drop sessions (cascades remaining tokens)
        return ResponseEntity.ok(Map.of("success", true, "user_id", userId.toString()));
    }

    @GetMapping("/factors")
    public ResponseEntity<List<FactorResponse>> listFactors(@PathVariable("id") UUID userId) {
        requireUser(userId);
        List<FactorResponse> factors = mfaFactorRepository.findByUserId(userId).stream()
                .map(this::toFactorResponse)
                .toList();
        return ResponseEntity.ok(factors);
    }

    @DeleteMapping("/factors/{factorId}")
    @Transactional
    public ResponseEntity<Map<String, Object>> deleteFactor(
            @PathVariable("id") UUID userId,
            @PathVariable("factorId") UUID factorId) {
        requireUser(userId);
        MfaFactor factor = mfaFactorRepository.findByIdAndUserId(factorId, userId)
                .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND, "MFA factor not found"));
        mfaFactorRepository.delete(factor);
        return ResponseEntity.ok(Map.of("success", true, "id", factorId.toString()));
    }

    private User requireUser(UUID userId) {
        return userRepository.findById(userId)
                .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND, "User not found"));
    }

    private Map<String, Object> toSessionMap(Session s) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("id", s.getId().toString());
        m.put("aal", s.getAal());
        m.put("ip", s.getIp());
        m.put("user_agent", s.getUserAgent());
        m.put("not_after", s.getNotAfter());
        m.put("refreshed_at", s.getRefreshedAt());
        m.put("created_at", s.getCreatedAt());
        m.put("updated_at", s.getUpdatedAt());
        return m;
    }

    private FactorResponse toFactorResponse(MfaFactor f) {
        return FactorResponse.builder()
                .id(f.getId().toString())
                .friendlyName(f.getFriendlyName())
                .factorType(f.getFactorType())
                .status(f.getStatus())
                .phone(f.getPhone())
                .createdAt(f.getCreatedAt())
                .updatedAt(f.getUpdatedAt())
                .build();
    }
}
