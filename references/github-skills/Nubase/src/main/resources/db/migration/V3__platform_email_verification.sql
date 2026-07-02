-- Email-OTP verification for platform (Studio) developer accounts.
-- Adds a verification timestamp to platform_users and a small table for one-time codes.

ALTER TABLE platform_users ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ;

-- Existing accounts predate verification — treat them as already verified so they are not
-- locked out when the new login gate ships.
UPDATE platform_users SET email_verified_at = NOW() WHERE email_verified_at IS NULL;

-- One-time numeric codes for platform signup / login / password-change confirmation.
-- Only the SHA-256 hash of the code is stored, never the plaintext.
CREATE TABLE IF NOT EXISTS platform_one_time_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email       VARCHAR(255) NOT NULL,
    purpose     VARCHAR(32)  NOT NULL,   -- 'signup' | 'login' | 'password_change'
    token_hash  VARCHAR(128) NOT NULL,   -- SHA-256 hex of the code
    expires_at  TIMESTAMPTZ  NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_platform_ott_email_purpose UNIQUE (email, purpose)
);

CREATE INDEX IF NOT EXISTS idx_platform_ott_email_purpose
    ON platform_one_time_tokens (email, purpose);
