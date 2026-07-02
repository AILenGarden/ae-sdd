package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.UpdateUserRequest;
import ai.nubase.auth.dto.response.UserResponse;
import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.util.UserMapper;
import lombok.RequiredArgsConstructor;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@Service
@RequiredArgsConstructor
public class UserService {

    private final UserRepository userRepository;
    private final IdentityRepository identityRepository;
    private final PasswordService passwordService;
    private final EmailService emailService;
    private final TokenGenerator tokenGenerator;
    private final UserMapper userMapper;
    private final EffectiveAuthConfig effectiveAuthConfig;
    private final OtpService otpService;

    /**
     * Get user by ID
     */
    @Transactional(readOnly = true)
    public User getUserById(UUID userId) {
        return userRepository.findById(userId)
                .orElseThrow(() -> new RuntimeException("User not found"));
    }

    /**
     * Get user by email
     */
    @Transactional(readOnly = true)
    public User getUserByEmail(String email) {
        return userRepository.findByEmail(email)
                .orElseThrow(() -> new RuntimeException("User not found"));
    }

    /**
     * Get current user response
     */
    @Transactional(readOnly = true)
    public UserResponse getCurrentUser(UUID userId) {
        User user = getUserById(userId);
        List<Identity> identities = identityRepository.findByUserId(userId);
        return userMapper.toUserResponse(user, identities);
    }

    /**
     * Update user
     */
    @Transactional
    public UserResponse updateUser(UUID userId, UpdateUserRequest request) {
        User user = getUserById(userId);

        // Update email (requires confirmation)
        if (request.getEmail() != null && !request.getEmail().equals(user.getEmail())) {
            // Check if new email is already taken
            if (userRepository.existsByEmail(request.getEmail())) {
                throw new RuntimeException("Email already in use");
            }

            String token = tokenGenerator.generateSecureToken();
            user.setEmailChangeTokenNew(token);
            user.setEmailChange(request.getEmail());
            user.setEmailChangeSentAt(Instant.now());
            user.setEmailChangeConfirmStatus((short) 0);

            emailService.sendEmailChangeConfirmation(user, request.getEmail(), token);
        }

        // Update password
        if (request.getPassword() != null && !request.getPassword().isBlank()) {
            // Optionally require a fresh reauthentication nonce (GoTrue secure password change).
            if (effectiveAuthConfig.password().isRequireReauthentication()) {
                if (!verifyReauthNonce(user, request.getNonce())) {
                    throw new RuntimeException("Reauthentication required: a valid nonce is required to change the password");
                }
                // consume the nonce
                user.setReauthenticationToken(null);
                user.setReauthenticationSentAt(null);
            }
            passwordService.validatePasswordStrength(request.getPassword());
            String hashedPassword = passwordService.hashPassword(request.getPassword());
            user.setEncryptedPassword(hashedPassword);
        }

        // Update phone (set as unconfirmed and send an OTP to verify ownership)
        if (request.getPhone() != null && !request.getPhone().equals(user.getPhone())) {
            user.setPhone(request.getPhone());
            user.setPhoneConfirmedAt(null); // Require re-confirmation
            user = userRepository.save(user); // persist before issuing the OTP
            otpService.sendPhoneChangeOtp(user, request.getPhone());
        }

        // Update user metadata
        if (request.getData() != null) {
            Map<String, Object> currentMetadata = user.getRawUserMetaData();
            if (currentMetadata == null) {
                user.setRawUserMetaData(request.getData());
            } else {
                currentMetadata.putAll(request.getData());
                user.setRawUserMetaData(currentMetadata);
            }
        }

        user = userRepository.save(user);
        List<Identity> identities = identityRepository.findByUserId(userId);

        return userMapper.toUserResponse(user, identities);
    }

    /** Validate a reauthentication nonce against the user's stored (hashed) token. */
    private boolean verifyReauthNonce(User user, String nonce) {
        if (StringUtils.isBlank(nonce) || user.getReauthenticationToken() == null) {
            return false;
        }
        if (user.getReauthenticationSentAt() != null) {
            Instant expiry = user.getReauthenticationSentAt()
                    .plus(effectiveAuthConfig.otp().getExpiration(), ChronoUnit.SECONDS);
            if (Instant.now().isAfter(expiry)) {
                return false;
            }
        }
        return user.getReauthenticationToken().equals(tokenGenerator.sha256(nonce));
    }

    /**
     * List the identities (auth providers) linked to a user.
     */
    @Transactional(readOnly = true)
    public List<Identity> getIdentities(UUID userId) {
        return identityRepository.findByUserId(userId);
    }

    /**
     * Unlink an identity from a user. A user must retain at least one identity, so the
     * last remaining identity cannot be removed. Mirrors GoTrue's
     * {@code DELETE /user/identities/{identity_id}}.
     */
    @Transactional
    public UserResponse unlinkIdentity(UUID userId, UUID identityId) {
        List<Identity> identities = identityRepository.findByUserId(userId);
        if (identities.size() <= 1) {
            throw new IllegalArgumentException("Cannot unlink the last identity from a user");
        }
        Identity target = identities.stream()
                .filter(i -> i.getId().equals(identityId))
                .findFirst()
                .orElseThrow(() -> new RuntimeException("Identity not found"));

        identityRepository.delete(target);

        // Keep app_metadata.providers in sync with the remaining identities.
        User user = getUserById(userId);
        Map<String, Object> appMeta = user.getRawAppMetaData();
        if (appMeta != null) {
            List<String> providers = identities.stream()
                    .filter(i -> !i.getId().equals(identityId))
                    .map(Identity::getProvider)
                    .distinct()
                    .toList();
            appMeta.put("providers", providers);
            if (!providers.isEmpty()) {
                appMeta.put("provider", providers.get(0));
            }
            user.setRawAppMetaData(appMeta);
            userRepository.save(user);
        }

        return userMapper.toUserResponse(user, identityRepository.findByUserId(userId));
    }
}
