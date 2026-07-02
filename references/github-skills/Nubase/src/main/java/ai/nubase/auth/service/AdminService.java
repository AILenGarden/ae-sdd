package ai.nubase.auth.service;

import ai.nubase.auth.dto.response.UserResponse;
import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.dto.request.admin.CreateUserRequest;
import ai.nubase.auth.dto.request.admin.InviteUserRequest;
import ai.nubase.auth.dto.request.admin.UpdateUserByIdRequest;
import ai.nubase.auth.dto.response.admin.ListUsersResponse;
import ai.nubase.auth.exception.EmailAlreadyExistsException;
import ai.nubase.auth.exception.UserNotFoundException;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.UserMapper;
import ai.nubase.common.enums.Role;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Pageable;
import org.springframework.data.domain.Sort;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.stream.Collectors;

/**
 * Admin service for managing users.
 * All methods require service_role authentication.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class AdminService {

    private final UserRepository userRepository;
    private final IdentityRepository identityRepository;
    private final PasswordService passwordService;
    private final EmailService emailService;
    private final TokenGenerator tokenGenerator;
    private final UserMapper userMapper;

    /**
     * Admin creates a user (supports auto-confirmation, password generation)
     */
    @Transactional
    public UserResponse createUser(CreateUserRequest request) {
        log.info("Admin creating user: {}", request.getEmail());

        // 1. Validate email uniqueness
        if (request.getEmail() != null && userRepository.existsByEmail(request.getEmail())) {
            throw new EmailAlreadyExistsException("User already registered");
        }

        // 2. Handle password (auto-generate if not provided)
        String password = request.getPassword();
        if (StringUtils.isBlank(password)) {
            password = tokenGenerator.generateSecureToken();
            log.debug("Auto-generated password for user: {}", request.getEmail());
        }
        String hashedPassword = passwordService.hashPassword(password);

        // 3. Build user entity
        // Determine role from either top-level field or app_metadata (supports both Supabase official and extended API)
        String role = determineRole(request.getRole(), request.getAppMetadata());

        // Prepare app_metadata with role synced
        Map<String, Object> appMetadata = prepareAppMetadata(request.getAppMetadata(), role);

        User user = User.builder()
                .email(request.getEmail())
                .encryptedPassword(hashedPassword)
                .phone(request.getPhone())
                .role(role)
                .aud(role)
                .rawUserMetaData(request.getUserMetadata() != null ? request.getUserMetadata() : new HashMap<>())
                .rawAppMetaData(appMetadata)
                .isSuperAdmin(false)
                .isSsoUser(false)
                .build();

        // 4. Handle email confirmation
        if (Boolean.TRUE.equals(request.getEmailConfirm())) {
            user.setEmailConfirmedAt(Instant.now());
            log.debug("Auto-confirmed email for user: {}", request.getEmail());
        } else {
            // Generate confirmation token but don't send email (Admin API controlled)
            String confirmationToken = tokenGenerator.generateSecureToken();
            user.setConfirmationToken(confirmationToken);
            user.setConfirmationSentAt(Instant.now());
        }

        // 5. Handle phone confirmation
        if (Boolean.TRUE.equals(request.getPhoneConfirm()) && request.getPhone() != null) {
            user.setPhoneConfirmedAt(Instant.now());
            log.debug("Auto-confirmed phone for user: {}", request.getEmail());
        }

        // 6. Handle ban logic
        if (request.getBanDuration() != null) {
            user.setBannedUntil(parseBanDuration(request.getBanDuration()));
            log.debug("Set ban duration for user {}: {}", request.getEmail(), request.getBanDuration());
        }

        user = userRepository.save(user);
        log.info("User created successfully: {} (ID: {})", user.getEmail(), user.getId());

        // 7. Create identity
        Identity identity = createIdentity(user, "email", request.getEmail());
        identityRepository.save(identity);

        // 8. Build response
        List<Identity> identities = identityRepository.findByUserId(user.getId());

        if(StringUtils.isNotBlank(user.getConfirmationToken())) {
            emailService.sendConfirmationEmail(user, user.getConfirmationToken(), null);
        }

        return userMapper.toUserResponse(user, identities);
    }

    /**
     * Admin updates user (supports forced confirmation, app_metadata modification)
     */
    @Transactional
    public UserResponse updateUserById(UUID userId, UpdateUserByIdRequest request) {
        log.info("Admin updating user: {}", userId);

        User user = userRepository.findById(userId)
                .orElseThrow(() -> new UserNotFoundException("User not found"));

        // 1. Update email (Admin can directly modify without confirmation)
        if (request.getEmail() != null && !request.getEmail().equals(user.getEmail())) {
            if (userRepository.existsByEmail(request.getEmail())) {
                throw new EmailAlreadyExistsException("Email already in use");
            }
            user.setEmail(request.getEmail());

            // Handle email confirmation status
            if (Boolean.TRUE.equals(request.getEmailConfirm())) {
                user.setEmailConfirmedAt(Instant.now());
            } else {
                user.setEmailConfirmedAt(null); // Reset confirmation status
            }
        } else if (request.getEmailConfirm() != null) {
            // Only update confirmation status if email wasn't changed
            if (Boolean.TRUE.equals(request.getEmailConfirm())) {
                user.setEmailConfirmedAt(Instant.now());
            } else {
                user.setEmailConfirmedAt(null);
            }
        }

        // 2. Update password
        if (request.getPassword() != null && !request.getPassword().isBlank()) {
            String hashedPassword = passwordService.hashPassword(request.getPassword());
            user.setEncryptedPassword(hashedPassword);
            log.debug("Updated password for user: {}", userId);
        }

        // 3. Update phone
        if (request.getPhone() != null) {
            user.setPhone(request.getPhone());
            if (Boolean.TRUE.equals(request.getPhoneConfirm())) {
                user.setPhoneConfirmedAt(Instant.now());
            } else {
                user.setPhoneConfirmedAt(null);
            }
        }

        // 4. Update role (supports both top-level role and app_metadata.role)
        String newRole = null;
        if (request.getRole() != null && !request.getRole().isBlank()) {
            // Top-level role takes precedence
            newRole = request.getRole();
        } else if (request.getAppMetadata() != null && request.getAppMetadata().containsKey("role")) {
            // Extract from app_metadata if no top-level role provided (Supabase official way)
            Object roleFromMetadata = request.getAppMetadata().get("role");
            if (roleFromMetadata != null) {
                newRole = roleFromMetadata.toString();
            }
        }

        if (newRole != null) {
            user.setRole(newRole);
            user.setAud(newRole); // Keep aud in sync with role
            log.debug("Updated role for user {}: {}", userId, newRole);
        }

        // 5. Update user_metadata (merge)
        if (request.getUserMetadata() != null) {
            Map<String, Object> currentMetadata = user.getRawUserMetaData();
            if (currentMetadata == null) {
                user.setRawUserMetaData(request.getUserMetadata());
            } else {
                currentMetadata.putAll(request.getUserMetadata());
                user.setRawUserMetaData(currentMetadata);
            }
            log.debug("Updated user_metadata for user: {}", userId);
        }

        // 6. Update app_metadata (Admin exclusive permission)
        if (request.getAppMetadata() != null) {
            Map<String, Object> currentAppMetadata = user.getRawAppMetaData();
            if (currentAppMetadata == null) {
                currentAppMetadata = new HashMap<>();
            }
            currentAppMetadata.putAll(request.getAppMetadata());

            // Sync role into app_metadata for JWT token compatibility
            if (newRole != null && !currentAppMetadata.containsKey("role")) {
                currentAppMetadata.put("role", newRole);
            }

            user.setRawAppMetaData(currentAppMetadata);
            log.debug("Updated app_metadata for user: {}", userId);
        } else if (newRole != null) {
            // If only role was updated but app_metadata wasn't provided, ensure role is synced
            Map<String, Object> currentAppMetadata = user.getRawAppMetaData();
            if (currentAppMetadata == null) {
                currentAppMetadata = new HashMap<>();
            }
            currentAppMetadata.put("role", newRole);
            user.setRawAppMetaData(currentAppMetadata);
        }

        // 7. Handle ban logic
        if (request.getBanDuration() != null) {
            user.setBannedUntil(parseBanDuration(request.getBanDuration()));
            log.debug("Updated ban duration for user {}: {}", userId, request.getBanDuration());
        }

        user = userRepository.save(user);
        log.info("User updated successfully: {}", userId);

        List<Identity> identities = identityRepository.findByUserId(userId);
        return userMapper.toUserResponse(user, identities);
    }

    /**
     * Admin deletes user (supports hard delete and soft delete)
     */
    @Transactional
    public UserResponse deleteUserById(UUID userId, boolean shouldSoftDelete) {
        log.info("Admin deleting user: {} (soft: {})", userId, shouldSoftDelete);

        User user = userRepository.findById(userId)
                .orElseThrow(() -> new UserNotFoundException("User not found"));

        List<Identity> identities = identityRepository.findByUserId(userId);
        UserResponse response = userMapper.toUserResponse(user, identities);

        if (shouldSoftDelete) {
            // Soft delete: set deleted_at
            user.setDeletedAt(Instant.now());
            userRepository.save(user);
            log.info("Soft deleted user: {}", userId);
        } else {
            // Hard delete: completely remove records
            identityRepository.deleteAll(identities); // Delete associated identities first
            userRepository.delete(user);
            log.info("Hard deleted user: {}", userId);
        }

        return response;
    }

    /**
     * Admin lists users (paginated with optional keyword search)
     */
    @Transactional(readOnly = true)
    public ListUsersResponse listUsers(int page, int perPage, String keyword) {
        log.debug("Admin listing users: page={}, perPage={}, keyword={}", page, perPage, keyword);

        // Parameter validation
        if (page < 1) page = 1;
        if (perPage < 1 || perPage > 100) perPage = 50;

        Pageable pageable = PageRequest.of(page - 1, perPage, Sort.by("createdAt").descending());

        // Use keyword search if provided, otherwise return all users
        Page<User> userPage;
        if (keyword != null && !keyword.strip().isEmpty()) {
            userPage = userRepository.searchByKeyword(keyword.strip(), pageable);
            log.debug("Searching users with keyword: {}", keyword);
        } else {
            userPage = userRepository.findAll(pageable);
        }

        List<UserResponse> userResponses = userPage.getContent().stream()
                .map(user -> {
                    List<Identity> identities = identityRepository.findByUserId(user.getId());
                    return userMapper.toUserResponse(user, identities);
                })
                .collect(Collectors.toList());

        String aud = Role.AUTHENTICATED.getValue();

        log.info("Listed {} users (page {}/{})", userResponses.size(), page, userPage.getTotalPages());

        return ListUsersResponse.builder()
                .users(userResponses)
                .aud(aud)
                .total(userPage.getTotalElements())
                .page(page)
                .perPage(perPage)
                .build();
    }

    /**
     * Admin gets user details
     */
    @Transactional(readOnly = true)
    public UserResponse getUserById(UUID userId) {
        log.debug("Admin getting user: {}", userId);

        User user = userRepository.findById(userId)
                .orElseThrow(() -> new UserNotFoundException("User not found"));

        List<Identity> identities = identityRepository.findByUserId(userId);
        return userMapper.toUserResponse(user, identities);
    }

    /**
     * Invite user (generate invitation link, send email)
     */
    @Transactional
    public UserResponse inviteUserByEmail(InviteUserRequest request) {
        log.info("Admin inviting user: {}", request.getEmail());

        // 1. Check if user already exists
        Optional<User> existingUser = userRepository.findByEmail(request.getEmail());
        if (existingUser.isPresent()) {
            User user = existingUser.get();
            if (user.getEmailConfirmedAt() != null) {
                throw new EmailAlreadyExistsException("User already registered");
            }
            // If user exists but not confirmed, resend invitation
            return resendInvite(user, request);
        }

        // 2. Create new user (unconfirmed state)
        String tempPassword = tokenGenerator.generateSecureToken();
        String hashedPassword = passwordService.hashPassword(tempPassword);

        String role = Role.AUTHENTICATED.getValue();
        User user = User.builder()
                .email(request.getEmail())
                .encryptedPassword(hashedPassword)
                .role(role)
                .aud(role)
                .rawUserMetaData(request.getData() != null ? request.getData() : new HashMap<>())
                .rawAppMetaData(createDefaultAppMetadata())
                .invitedAt(Instant.now()) // Set invitation time
                .isSuperAdmin(false)
                .isSsoUser(false)
                .build();

        // 3. Generate confirmation token
        String confirmationToken = tokenGenerator.generateSecureToken();
        user.setConfirmationToken(confirmationToken);
        user.setConfirmationSentAt(Instant.now());

        user = userRepository.save(user);
        log.info("Created invited user: {}", user.getEmail());

        // 4. Create identity
        Identity identity = createIdentity(user, "email", request.getEmail());
        identityRepository.save(identity);

        // 5. Send invitation email
        emailService.sendInvitationEmail(user, confirmationToken, request.getRedirectTo());
        log.info("Sent invitation email to: {}", request.getEmail());

        // 6. Build response
        List<Identity> identities = identityRepository.findByUserId(user.getId());
        return userMapper.toUserResponse(user, identities);
    }

    // ===== Helper Methods =====

    /**
     * Create default app metadata
     */
    private Map<String, Object> createDefaultAppMetadata() {
        Map<String, Object> metadata = new HashMap<>();
        metadata.put("provider", "email");
        metadata.put("providers", List.of("email"));
        return metadata;
    }

    /**
     * Create identity entity
     */
    private Identity createIdentity(User user, String provider, String providerId) {
        Map<String, Object> identityData = new HashMap<>();
        identityData.put("sub", user.getId().toString());
        identityData.put("email", user.getEmail());

        return Identity.builder()
                .user(user)
                .provider(provider)
                .providerId(providerId)
                .identityData(identityData)
                .lastSignInAt(Instant.now())
                .build();
    }

    /**
     * Parse ban duration string to Instant
     */
    private Instant parseBanDuration(String duration) {
        if ("none".equalsIgnoreCase(duration)) {
            return null;
        }
        if ("permanent".equalsIgnoreCase(duration)) {
            // Use a far-future timestamp Postgres' timestamptz can actually store —
            // Long.MAX_VALUE ms overflows PG's range and explodes the UPDATE.
            return Instant.parse("9999-12-31T23:59:59Z");
        }

        // Parse format like "24h", "7d", "30d"
        try {
            if (duration.endsWith("h")) {
                long hours = Long.parseLong(duration.substring(0, duration.length() - 1));
                return Instant.now().plus(hours, ChronoUnit.HOURS);
            } else if (duration.endsWith("d")) {
                long days = Long.parseLong(duration.substring(0, duration.length() - 1));
                return Instant.now().plus(days, ChronoUnit.DAYS);
            }
        } catch (NumberFormatException e) {
            log.warn("Invalid ban duration format: {}. Defaulting to 24 hours.", duration);
        }

        // Default: 24 hours
        return Instant.now().plus(24, ChronoUnit.HOURS);
    }

    /**
     * Resend invitation to existing unconfirmed user
     */
    private UserResponse resendInvite(User user, InviteUserRequest request) {
        log.info("Resending invitation to existing user: {}", user.getEmail());

        String confirmationToken = tokenGenerator.generateSecureToken();
        user.setConfirmationToken(confirmationToken);
        user.setConfirmationSentAt(Instant.now());
        user.setInvitedAt(Instant.now());

        // Update user_metadata if provided
        if (request.getData() != null) {
            user.setRawUserMetaData(request.getData());
        }

        user = userRepository.save(user);

        emailService.sendInvitationEmail(user, confirmationToken, request.getRedirectTo());
        log.info("Resent invitation email to: {}", user.getEmail());

        List<Identity> identities = identityRepository.findByUserId(user.getId());
        return userMapper.toUserResponse(user, identities);
    }

    /**
     * Determine user role from either top-level field or app_metadata
     * Supports both extended API (top-level role) and Supabase official API (app_metadata.role)
     *
     * Priority:
     * 1. Top-level role field (if provided)
     * 2. app_metadata.role (if provided)
     * 3. Default: authenticated
     */
    private String determineRole(String topLevelRole, Map<String, Object> appMetadata) {
        // Priority 1: Top-level role field
        if (topLevelRole != null && !topLevelRole.isBlank()) {
            return topLevelRole;
        }

        // Priority 2: app_metadata.role (Supabase official way)
        if (appMetadata != null && appMetadata.containsKey("role")) {
            Object roleFromMetadata = appMetadata.get("role");
            if (roleFromMetadata != null) {
                String role = roleFromMetadata.toString();
                if (!role.isBlank()) {
                    return role;
                }
            }
        }

        // Priority 3: Default role
        return Role.AUTHENTICATED.getValue();
    }

    /**
     * Prepare app_metadata with role synced for JWT token compatibility
     * Ensures that role is always available in app_metadata regardless of which way it was set
     */
    private Map<String, Object> prepareAppMetadata(Map<String, Object> appMetadata, String role) {
        Map<String, Object> result = appMetadata != null ? new HashMap<>(appMetadata) : createDefaultAppMetadata();

        // Always sync role into app_metadata for JWT token compatibility
        if (role != null && !role.isBlank()) {
            result.put("role", role);
        }

        return result;
    }
}
