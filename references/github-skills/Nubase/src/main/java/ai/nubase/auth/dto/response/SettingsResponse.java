package ai.nubase.auth.dto.response;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

/**
 * Response for {@code GET /auth/v1/settings} — advertises which auth methods/providers
 * are enabled for the tenant. The Supabase client reads this on startup. The shape mirrors
 * GoTrue's settings payload (external providers map + feature flags).
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class SettingsResponse {

    /** Map of provider name -> enabled (e.g. {"google": true, "github": false}). */
    private Map<String, Boolean> external;

    private boolean disableSignup;

    private boolean mailerAutoconfirm;

    private boolean phoneAutoconfirm;

    private String smsProvider;

    private boolean mfaEnabled;

    private boolean saml;
}
