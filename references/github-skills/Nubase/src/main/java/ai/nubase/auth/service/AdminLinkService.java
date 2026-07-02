package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.admin.GenerateLinkRequest;
import ai.nubase.auth.dto.response.admin.GenerateLinkResponse;
import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.OneTimeToken;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.OneTimeTokenRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.util.UserMapper;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import ai.nubase.common.enums.Role;
import lombok.RequiredArgsConstructor;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.web.util.UriComponentsBuilder;

import java.time.Instant;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Admin-only generation of verification action links WITHOUT sending an email
 * ({@code POST /auth/v1/admin/generate_link}). Mirrors Supabase GoTrue — useful when the
 * caller delivers the email itself. Supports the verification types the {@code /verify}
 * endpoint actually round-trips: signup, recovery, magiclink.
 */
@Service
@RequiredArgsConstructor
public class AdminLinkService {

    private final UserRepository userRepository;
    private final IdentityRepository identityRepository;
    private final OneTimeTokenRepository oneTimeTokenRepository;
    private final PasswordService passwordService;
    private final EffectiveAuthConfig effectiveAuthConfig;
    private final TokenGenerator tokenGenerator;
    private final UserMapper userMapper;
    private final AuthConfig authConfig;

    @Transactional
    public GenerateLinkResponse generate(GenerateLinkRequest req) {
        String email = req.getEmail();
        return switch (req.getType()) {
            case "recovery" -> recovery(email, req.getRedirectTo());
            case "signup" -> signup(email, req.getPassword(), req.getData(), req.getRedirectTo());
            case "magiclink" -> magiclink(email, req.getRedirectTo());
            default -> throw new IllegalArgumentException(
                    "Unsupported generate_link type: " + req.getType() + " (supported: signup, recovery, magiclink)");
        };
    }

    private GenerateLinkResponse recovery(String email, String redirectTo) {
        User user = userRepository.findByEmail(email)
                .orElseThrow(() -> new RuntimeException("User not found"));
        String token = tokenGenerator.generateSecureToken();
        user.setRecoveryToken(token);
        user.setRecoverySentAt(Instant.now());
        userRepository.save(user);
        return build("recovery", token, null, email, redirectTo, user);
    }

    private GenerateLinkResponse signup(String email, String password,
                                        Map<String, Object> data, String redirectTo) {
        User user = userRepository.findByEmail(email).orElse(null);
        if (user == null) {
            String role = Role.AUTHENTICATED.getValue();
            User.UserBuilder b = User.builder()
                    .email(email).role(role).aud(role)
                    .rawUserMetaData(data != null ? data : new HashMap<>())
                    .rawAppMetaData(appMeta("email"))
                    .isSuperAdmin(false).isSsoUser(false).isAnonymous(false);
            if (StringUtils.isNotBlank(password)) {
                passwordService.validatePasswordStrength(password);
                b.encryptedPassword(passwordService.hashPassword(password));
            }
            user = userRepository.save(b.build());
            identityRepository.save(identity(user, email));
        }
        String token = tokenGenerator.generateSecureToken();
        user.setConfirmationToken(token);
        user.setConfirmationSentAt(Instant.now());
        userRepository.save(user);
        return build("signup", token, null, email, redirectTo, user);
    }

    private GenerateLinkResponse magiclink(String email, String redirectTo) {
        User user = userRepository.findByEmail(email).orElse(null);
        if (user == null) {
            String role = Role.AUTHENTICATED.getValue();
            user = userRepository.save(User.builder()
                    .email(email).role(role).aud(role)
                    .rawUserMetaData(new HashMap<>()).rawAppMetaData(appMeta("email"))
                    .isSuperAdmin(false).isSsoUser(false).isAnonymous(false)
                    .build());
            identityRepository.save(identity(user, email));
        }
        String linkToken = tokenGenerator.generateSecureToken();
        String otpCode = tokenGenerator.generateNumericOTP(effectiveAuthConfig.otp().getLength());
        storeOtt(user, OneTimeToken.TYPE_MAGICLINK, linkToken, email);
        storeOtt(user, OneTimeToken.TYPE_OTP, otpCode, email);
        return build("magiclink", linkToken, otpCode, email, redirectTo, user);
    }

    private GenerateLinkResponse build(String type, String token, String otp,
                                       String email, String redirectTo, User user) {
        String base = authConfig.getApp().getDomain(MultiTenancyContext.getAppCode()) + "/auth/v1/verify";
        UriComponentsBuilder uri = UriComponentsBuilder.fromUriString(base)
                .queryParam("token", token)
                .queryParam("type", type)
                .queryParam("email", email)
                .queryParam("apikey", MultiTenancyContext.getApiKey());
        if (StringUtils.isNotBlank(redirectTo)) {
            uri.queryParam("redirect_to", redirectTo);
        }
        List<Identity> identities = identityRepository.findByUserId(user.getId());
        return GenerateLinkResponse.builder()
                .actionLink(uri.build().toUriString())
                .token(token)
                .emailOtp(otp)
                .verificationType(type)
                .redirectTo(redirectTo)
                .user(userMapper.toUserResponse(user, identities))
                .build();
    }

    private void storeOtt(User user, String type, String value, String relatesTo) {
        oneTimeTokenRepository.deleteByUserIdAndTokenType(user.getId(), type);
        oneTimeTokenRepository.save(OneTimeToken.builder()
                .user(user).tokenType(type)
                .tokenHash(tokenGenerator.sha256(value))
                .relatesTo(relatesTo)
                .build());
    }

    private Identity identity(User user, String email) {
        Map<String, Object> data = new HashMap<>();
        data.put("sub", user.getId().toString());
        data.put("email", email);
        return Identity.builder()
                .user(user).provider("email").providerId(user.getId().toString())
                .identityData(data).lastSignInAt(Instant.now())
                .build();
    }

    private Map<String, Object> appMeta(String provider) {
        Map<String, Object> m = new HashMap<>();
        m.put("provider", provider);
        m.put("providers", List.of(provider));
        return m;
    }
}
