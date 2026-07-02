package ai.nubase.auth.controller;

import ai.nubase.auth.dto.request.UpdateUserRequest;
import ai.nubase.auth.dto.response.UserResponse;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.service.UserService;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

/**
 * User Controller
 * Handles user management endpoints
 * Base path: /auth/v1 (configured in application.yml)
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1")
public class UserController {

    private final UserService userService;

    /**
     * Get current user information
     * GET /auth/v1/user
     */
    @GetMapping("/user")
    public ResponseEntity<UserResponse> getUser(@AuthenticationPrincipal User user) {
        UserResponse response = userService.getCurrentUser(user.getId());
        return ResponseEntity.ok(response);
    }

    /**
     * Update current user
     * PUT /auth/v1/user
     */
    @PutMapping("/user")
    public ResponseEntity<UserResponse> updateUser(
            @AuthenticationPrincipal User user,
            @Valid @RequestBody UpdateUserRequest request) {
        UserResponse response = userService.updateUser(user.getId(), request);
        return ResponseEntity.ok(response);
    }

    /**
     * Unlink an identity (auth provider) from the current user.
     * DELETE /auth/v1/user/identities/{identityId}
     */
    @DeleteMapping("/user/identities/{identityId}")
    public ResponseEntity<UserResponse> unlinkIdentity(
            @AuthenticationPrincipal User user,
            @PathVariable("identityId") java.util.UUID identityId) {
        UserResponse response = userService.unlinkIdentity(user.getId(), identityId);
        return ResponseEntity.ok(response);
    }
}
