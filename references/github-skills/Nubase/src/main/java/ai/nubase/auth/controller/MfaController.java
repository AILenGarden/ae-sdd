package ai.nubase.auth.controller;

import ai.nubase.auth.dto.request.mfa.EnrollFactorRequest;
import ai.nubase.auth.dto.request.mfa.VerifyFactorRequest;
import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.dto.response.mfa.ChallengeResponse;
import ai.nubase.auth.dto.response.mfa.EnrollFactorResponse;
import ai.nubase.auth.dto.response.mfa.FactorResponse;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.service.JwtSecretService;
import ai.nubase.auth.service.MfaService;
import io.jsonwebtoken.Claims;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

/**
 * Multi-Factor Authentication endpoints (Supabase GoTrue parity).
 * Base path: /auth/v1
 *
 * <ul>
 *   <li>GET    /factors                 — list enrolled factors</li>
 *   <li>POST   /factors                 — enroll a factor (TOTP / phone)</li>
 *   <li>POST   /factors/{id}/challenge  — create a challenge</li>
 *   <li>POST   /factors/{id}/verify     — verify a code, upgrade session to AAL2</li>
 *   <li>DELETE /factors/{id}            — unenroll a factor</li>
 * </ul>
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1/factors")
public class MfaController {

    private final MfaService mfaService;
    private final JwtSecretService jwtSecretService;

    @GetMapping
    public ResponseEntity<List<FactorResponse>> listFactors(@AuthenticationPrincipal User user) {
        return ResponseEntity.ok(mfaService.listFactors(user));
    }

    @PostMapping
    public ResponseEntity<EnrollFactorResponse> enroll(
            @AuthenticationPrincipal User user,
            @RequestBody EnrollFactorRequest request) {
        EnrollFactorResponse response = mfaService.enroll(
                user, request.getFactorType(), request.getFriendlyName(), request.getPhone());
        return ResponseEntity.ok(response);
    }

    @PostMapping("/{id}/challenge")
    public ResponseEntity<ChallengeResponse> challenge(
            @AuthenticationPrincipal User user,
            @PathVariable("id") String factorId) {
        return ResponseEntity.ok(mfaService.challenge(user, factorId));
    }

    @PostMapping("/{id}/verify")
    public ResponseEntity<AuthResponse> verify(
            HttpServletRequest httpRequest,
            @AuthenticationPrincipal User user,
            @PathVariable("id") String factorId,
            @Valid @RequestBody VerifyFactorRequest request) {
        String sessionId = currentSessionId(httpRequest);
        AuthResponse response = mfaService.verify(
                user, factorId, request.getChallengeId(), request.getCode(), sessionId);
        return ResponseEntity.ok(response);
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Map<String, String>> unenroll(
            @AuthenticationPrincipal User user,
            @PathVariable("id") String factorId) {
        String id = mfaService.unenroll(user, factorId);
        return ResponseEntity.ok(Map.of("id", id));
    }

    /** Extract the session_id claim from the caller's Bearer access token. */
    private String currentSessionId(HttpServletRequest request) {
        String bearer = request.getHeader("Authorization");
        if (bearer == null || !bearer.startsWith("Bearer ")) {
            return null;
        }
        try {
            Claims claims = jwtSecretService.validateToken(bearer.substring(7));
            return claims.get("session_id", String.class);
        } catch (Exception e) {
            return null;
        }
    }
}
