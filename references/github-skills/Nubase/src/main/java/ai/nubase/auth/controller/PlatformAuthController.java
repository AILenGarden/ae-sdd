package ai.nubase.auth.controller;

import ai.nubase.auth.dto.request.platform.PlatformPasswordChangeRequest;
import ai.nubase.auth.dto.request.platform.PlatformPasswordOtpRequest;
import ai.nubase.auth.dto.request.platform.PlatformResendRequest;
import ai.nubase.auth.dto.request.platform.PlatformSignInRequest;
import ai.nubase.auth.dto.request.platform.PlatformSignUpRequest;
import ai.nubase.auth.dto.request.platform.PlatformVerifyEmailRequest;
import ai.nubase.auth.dto.response.platform.PlatformAuthResponse;
import ai.nubase.auth.dto.response.platform.PlatformPendingResponse;
import ai.nubase.auth.dto.response.platform.PlatformUserPayload;
import ai.nubase.auth.exception.EmailAlreadyExistsException;
import ai.nubase.auth.service.PlatformAuthService;
import ai.nubase.auth.service.PlatformAuthService.AuthOutcome;
import ai.nubase.auth.service.PlatformOAuthService;
import ai.nubase.auth.service.RateLimiterService.RateLimitExceededException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.Map;
import java.util.UUID;

/**
 * Endpoints for platform-level developer accounts (Studio login).
 * Does not depend on tenant context — uses the metadata database.
 */
@RestController
@RequestMapping("/auth/v1/platform")
@RequiredArgsConstructor
@Slf4j
public class PlatformAuthController {

    private final PlatformAuthService platformAuthService;
    private final PlatformOAuthService platformOAuthService;

    /**
     * GET /auth/v1/platform/config — public config the Studio frontend needs to render the
     * login/signup page correctly (incl. which OAuth providers are enabled). No auth required.
     */
    @GetMapping("/config")
    public ResponseEntity<?> publicConfig() {
        return ResponseEntity.ok(Map.of(
                "signup_enabled", platformAuthService.isSignupEnabled(),
                "google_enabled", platformOAuthService.googleEnabled(),
                "google_code_enabled", platformOAuthService.googleCodeEnabled(),
                "github_enabled", platformOAuthService.githubEnabled(),
                "google_client_id", platformOAuthService.googleClientId()
        ));
    }

    @PostMapping("/signup")
    public ResponseEntity<?> signUp(@Valid @RequestBody PlatformSignUpRequest request) {
        try {
            AuthOutcome outcome = platformAuthService.signUp(request);
            if (outcome.pending()) {
                return ResponseEntity.accepted()
                        .body(PlatformPendingResponse.builder().email(outcome.pendingEmail()).build());
            }
            return ResponseEntity.status(HttpStatus.CREATED).body(outcome.token());
        } catch (EmailAlreadyExistsException e) {
            return ResponseEntity.status(HttpStatus.UNPROCESSABLE_ENTITY)
                    .body(Map.of("error", "user_exists", "message", e.getMessage()));
        } catch (RateLimitExceededException e) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(Map.of("error", "rate_limited", "message", e.getMessage()));
        } catch (Exception e) {
            log.error("Platform signup failed", e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(Map.of("error", "signup_failed", "message", "Could not complete sign up. Please try again."));
        }
    }

    @PostMapping("/token")
    public ResponseEntity<?> token(@Valid @RequestBody PlatformSignInRequest request) {
        try {
            AuthOutcome outcome = platformAuthService.signIn(request);
            if (outcome.pending()) {
                return ResponseEntity.accepted()
                        .body(PlatformPendingResponse.builder().email(outcome.pendingEmail()).build());
            }
            return ResponseEntity.ok(outcome.token());
        } catch (RateLimitExceededException e) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(Map.of("error", "rate_limited", "message", e.getMessage()));
        } catch (IllegalArgumentException e) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED)
                    .body(Map.of("error", "invalid_credentials", "message", e.getMessage()));
        } catch (Exception e) {
            log.error("Platform sign-in failed", e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(Map.of("error", "signin_failed", "message", "Sign in failed. Please try again."));
        }
    }

    /** Confirm a signup/login email code and issue a session. */
    @PostMapping("/verify-email")
    public ResponseEntity<?> verifyEmail(@Valid @RequestBody PlatformVerifyEmailRequest request) {
        try {
            PlatformAuthResponse response =
                    platformAuthService.verifyEmail(request.getEmail(), request.getCode());
            return ResponseEntity.ok(response);
        } catch (RateLimitExceededException e) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(Map.of("error", "rate_limited", "message", e.getMessage()));
        } catch (IllegalArgumentException e) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED)
                    .body(Map.of("error", "invalid_code", "message", e.getMessage()));
        } catch (Exception e) {
            log.error("Platform email verification failed", e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(Map.of("error", "verify_failed", "message", "Verification failed. Please try again."));
        }
    }

    /** Re-send a verification code. Always 200 (no account enumeration). */
    @PostMapping("/verify-email/resend")
    public ResponseEntity<?> resendVerification(@Valid @RequestBody PlatformResendRequest request) {
        try {
            platformAuthService.resendVerification(request.getEmail());
        } catch (RateLimitExceededException e) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(Map.of("error", "rate_limited", "message", e.getMessage()));
        } catch (Exception e) {
            log.warn("Platform verification resend failed: {}", e.getMessage());
        }
        return ResponseEntity.ok(Map.of("ok", true));
    }

    /** Step 1 of password change (Bearer): verify current password, email a code. */
    @PostMapping("/password/otp")
    public ResponseEntity<?> requestPasswordOtp(HttpServletRequest httpRequest,
                                                @Valid @RequestBody PlatformPasswordOtpRequest request) {
        UUID userId = authenticate(httpRequest);
        if (userId == null) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED).body(Map.of("error", "invalid_token"));
        }
        try {
            platformAuthService.requestPasswordOtp(userId, request.getCurrentPassword());
            return ResponseEntity.ok(Map.of("ok", true));
        } catch (RateLimitExceededException e) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(Map.of("error", "rate_limited", "message", e.getMessage()));
        } catch (IllegalArgumentException e) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "invalid_password", "message", e.getMessage()));
        } catch (Exception e) {
            log.error("Platform password OTP request failed", e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(Map.of("error", "request_failed", "message", "Could not start the password change. Please try again."));
        }
    }

    /** Step 2 of password change (Bearer): current password + new password + emailed code. */
    @PostMapping("/password")
    public ResponseEntity<?> changePassword(HttpServletRequest httpRequest,
                                            @Valid @RequestBody PlatformPasswordChangeRequest request) {
        UUID userId = authenticate(httpRequest);
        if (userId == null) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED).body(Map.of("error", "invalid_token"));
        }
        try {
            platformAuthService.changePassword(userId, request.getCurrentPassword(),
                    request.getNewPassword(), request.getCode());
            return ResponseEntity.ok(Map.of("ok", true));
        } catch (RateLimitExceededException e) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(Map.of("error", "rate_limited", "message", e.getMessage()));
        } catch (IllegalArgumentException e) {
            return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                    .body(Map.of("error", "change_failed", "message", e.getMessage()));
        } catch (Exception e) {
            log.error("Platform password change failed", e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(Map.of("error", "change_failed", "message", "Could not change the password. Please try again."));
        }
    }

    @GetMapping("/me")
    public ResponseEntity<?> me(HttpServletRequest request) {
        UUID userId = authenticate(request);
        if (userId == null) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED).body(Map.of("error", "missing_token"));
        }
        try {
            PlatformUserPayload payload = platformAuthService.describe(userId);
            return ResponseEntity.ok(payload);
        } catch (Exception e) {
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED)
                    .body(Map.of("error", "invalid_token", "message", e.getMessage()));
        }
    }

    /** Resolve the platform user id from the Bearer token, or null if missing/invalid. */
    private UUID authenticate(HttpServletRequest request) {
        String authHeader = request.getHeader("Authorization");
        if (StringUtils.isBlank(authHeader) || !authHeader.startsWith("Bearer ")) {
            return null;
        }
        try {
            return platformAuthService.validateAndGetSubject(authHeader.substring(7));
        } catch (Exception e) {
            return null;
        }
    }
}
