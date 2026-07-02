# Security Policy

## Supported Versions

Nubase is pre-1.0 and under active development. Security fixes are applied to
the latest `main`. Until a stable release line exists, please always run the
most recent commit.

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

Report privately through GitHub's built-in private vulnerability reporting:

1. Go to the **Security** tab of this repository.
2. Click **Report a vulnerability**.
3. Provide a description, reproduction steps, affected version/commit, and the
   potential impact.

We aim to acknowledge reports within 5 business days and to provide a remediation
timeline after triage. Please give us a reasonable window to release a fix before
any public disclosure.

## Scope

In-scope examples:

- Authentication/authorization bypass (JWT handling, RLS, role escalation).
- Cross-tenant data access (one project reading another project's database,
  memory, or storage).
- Leakage of secrets or encrypted credentials.
- Injection (SQL, header, path) reachable through the REST, Auth, Memory, or
  Storage APIs.
- Remote code execution or SSRF.

Out of scope:

- Issues that require a pre-compromised host or operator-level access.
- Missing hardening on a deployment the operator chose to expose without the
  documented secrets/configuration.
- Denial of service from unbounded local resource use in a self-hosted instance.

## Hardening Notes for Self-Hosters

Nubase is a self-hostable backend. A misconfigured deployment can expose data.
When running outside local development:

- **Set strong, stable secrets via environment.** `PGRST_ENCRYPTION_MASTER_KEY`
  encrypts per-project DB passwords and JWT secrets; the metadata service-role
  key is a full-access admin credential. Never use development defaults in
  production, and rotate leaked keys immediately.
- **Treat the service-role key like a root password.** It bypasses Row Level
  Security. Keep it server-side only.
- **Do not expose the SQL/admin surfaces to the public internet** without an
  authentication layer in front. The SQL editor and MCP database tools can run
  arbitrary SQL.
- **Lock down CORS** to the origins you control.
- **Use TLS** in front of the backend and Studio.
- **Protect storage credentials** (R2/S3/MinIO keys) and scope buckets correctly.

See `docs/` for configuration and architecture details.
