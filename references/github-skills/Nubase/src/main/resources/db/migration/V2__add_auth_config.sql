-- Per-tenant authentication settings (JSON overrides over the global nubase.auth.* config).
-- Stored alongside the existing per-tenant oauth_config. NULL → tenant uses global defaults.
ALTER TABLE database_configs ADD COLUMN IF NOT EXISTS auth_config TEXT;
COMMENT ON COLUMN database_configs.auth_config IS
    'Per-tenant auth settings as JSON (TenantAuthConfig): overrides global nubase.auth.* config';
