package ai.nubase.postgrest.multidb;

import javax.crypto.Cipher;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.Scanner;

/**
 * Command-line tool for generating encrypted passwords and JWT secrets
 *
 * Usage:
 *   # Generate a new master key
 *
 *   # Encrypt a password using master key
 *
 *   # Interactive mode
 */
public class EncryptionTool {

    private static final String ALGORITHM = "AES/GCM/NoPadding";
    private static final int GCM_TAG_LENGTH = 128;
    private static final int GCM_IV_LENGTH = 12;
    private static final String ENCRYPTED_PREFIX = "ENCRYPTED:AES256:";
    private static final SecureRandom secureRandom = new SecureRandom();

    /**
     * Generate a random 256-bit encryption key
     */
    public static String generateMasterKey() {
        byte[] keyBytes = new byte[32]; // 256 bits
        secureRandom.nextBytes(keyBytes);
        return Base64.getEncoder().encodeToString(keyBytes);
    }

    /**
     * Encrypt plaintext using the provided master key
     */
    public static String encrypt(String masterKeyBase64, String plaintext) throws Exception {
        if (plaintext == null || plaintext.isBlank()) {
            throw new IllegalArgumentException("Plaintext cannot be null or blank");
        }

        // Decode master key
        byte[] keyBytes = Base64.getDecoder().decode(masterKeyBase64);
        if (keyBytes.length != 32) {
            throw new IllegalArgumentException("Master key must be 32 bytes (256 bits)");
        }
        SecretKey masterKey = new SecretKeySpec(keyBytes, "AES");

        // Generate random IV
        byte[] iv = new byte[GCM_IV_LENGTH];
        secureRandom.nextBytes(iv);

        // Initialize cipher
        Cipher cipher = Cipher.getInstance(ALGORITHM);
        GCMParameterSpec parameterSpec = new GCMParameterSpec(GCM_TAG_LENGTH, iv);
        cipher.init(Cipher.ENCRYPT_MODE, masterKey, parameterSpec);

        // Encrypt
        byte[] ciphertext = cipher.doFinal(plaintext.getBytes(StandardCharsets.UTF_8));

        // Combine IV + ciphertext
        byte[] combined = new byte[iv.length + ciphertext.length];
        System.arraycopy(iv, 0, combined, 0, iv.length);
        System.arraycopy(ciphertext, 0, combined, iv.length, ciphertext.length);

        // Encode and add prefix
        String encoded = Base64.getEncoder().encodeToString(combined);
        return ENCRYPTED_PREFIX + encoded;
    }

    /**
     * Decrypt encrypted text using the provided master key
     */
    public static String decrypt(String masterKeyBase64, String encryptedText) throws Exception {
        if (!encryptedText.startsWith(ENCRYPTED_PREFIX)) {
            throw new IllegalArgumentException("Invalid encrypted text format");
        }

        // Decode master key
        byte[] keyBytes = Base64.getDecoder().decode(masterKeyBase64);
        SecretKey masterKey = new SecretKeySpec(keyBytes, "AES");

        // Remove prefix and decode
        String base64Data = encryptedText.substring(ENCRYPTED_PREFIX.length());
        byte[] combined = Base64.getDecoder().decode(base64Data);

        // Extract IV and ciphertext
        byte[] iv = new byte[GCM_IV_LENGTH];
        byte[] ciphertext = new byte[combined.length - GCM_IV_LENGTH];
        System.arraycopy(combined, 0, iv, 0, iv.length);
        System.arraycopy(combined, iv.length, ciphertext, 0, ciphertext.length);

        // Initialize cipher
        Cipher cipher = Cipher.getInstance(ALGORITHM);
        GCMParameterSpec parameterSpec = new GCMParameterSpec(GCM_TAG_LENGTH, iv);
        cipher.init(Cipher.DECRYPT_MODE, masterKey, parameterSpec);

        // Decrypt
        byte[] plaintext = cipher.doFinal(ciphertext);
        return new String(plaintext, StandardCharsets.UTF_8);
    }

    /**
     * Interactive mode
     */
    private static void interactiveMode() {
        Scanner scanner = new Scanner(System.in);

        System.out.println("╔════════════════════════════════════════════════════════════╗");
        System.out.println("║   PostgREST Multi-Database Encryption Tool                ║");
        System.out.println("╚════════════════════════════════════════════════════════════╝");
        System.out.println();

        while (true) {
            System.out.println("Options:");
            System.out.println("  1. Generate new master key");
            System.out.println("  2. Encrypt password/JWT secret");
            System.out.println("  3. Decrypt value (for verification)");
            System.out.println("  4. Generate complete database config");
            System.out.println("  0. Exit");
            System.out.print("\nSelect option: ");

            String option = scanner.nextLine().strip();

            try {
                switch (option) {
                    case "1" -> generateKeyInteractive();
                    case "2" -> encryptInteractive(scanner);
                    case "3" -> decryptInteractive(scanner);
                    case "4" -> generateConfigInteractive();
                    case "0" -> {
                        System.out.println("\nGoodbye!");
                        return;
                    }
                    default -> System.out.println("\nInvalid option. Please try again.\n");
                }
            } catch (Exception e) {
                System.err.println("\nError: " + e.getMessage());
                System.out.println();
            }
        }
    }

    private static void generateKeyInteractive() {
        System.out.println("\n" + "=".repeat(60));
        System.out.println("GENERATING NEW MASTER KEY");
        System.out.println("=".repeat(60));

        String masterKey = generateMasterKey();

        System.out.println("\n✓ Master Key Generated:");
        System.out.println(masterKey);
        System.out.println("\n⚠️  IMPORTANT: Save this key securely!");
        System.out.println("\nTo use this key, set it as environment variable:");
        System.out.println("export PGRST_ENCRYPTION_MASTER_KEY=\"" + masterKey + "\"");
        System.out.println();
    }

    private static void encryptInteractive(Scanner scanner) throws Exception {
        System.out.println("\n" + "=".repeat(60));
        System.out.println("ENCRYPT PASSWORD/SECRET");
        System.out.println("=".repeat(60));

        System.out.print("\nEnter master key (Base64): ");
        String masterKey = scanner.nextLine().strip();

        System.out.print("Enter plaintext to encrypt: ");
        String plaintext = scanner.nextLine().strip();

        String encrypted = encrypt(masterKey, plaintext);

        System.out.println("\n✓ Encrypted Value:");
        System.out.println(encrypted);
        System.out.println("\n💡 Use this value in database_configs table");
        System.out.println();
    }

    private static void decryptInteractive(Scanner scanner) throws Exception {
        System.out.println("\n" + "=".repeat(60));
        System.out.println("DECRYPT VALUE (VERIFICATION)");
        System.out.println("=".repeat(60));

        System.out.print("\nEnter master key (Base64): ");
        String masterKey = scanner.nextLine().strip();

        System.out.print("Enter encrypted value: ");
        String encrypted = scanner.nextLine().strip();

        String decrypted = decrypt(masterKey, encrypted);

        System.out.println("\n✓ Decrypted Value:");
        System.out.println(decrypted);
        System.out.println();
    }

    private static void generateConfigInteractive() throws Exception {
        System.out.println("\n" + "=".repeat(60));
        System.out.println("GENERATE COMPLETE DATABASE CONFIG");
        System.out.println("=".repeat(60));

        java.io.BufferedReader in = new java.io.BufferedReader(
                new java.io.InputStreamReader(System.in));

        String masterKey = prompt(in, "Master key (env PGRST_ENCRYPTION_MASTER_KEY) " +
                "or blank to generate one");
        if (masterKey.isEmpty()) {
            masterKey = System.getenv("PGRST_ENCRYPTION_MASTER_KEY");
        }
        if (masterKey == null || masterKey.isEmpty()) {
            masterKey = generateMasterKey();
            System.out.println("\n✓ Generated new master key:");
            System.out.println(masterKey);
            System.out.println("\nSave this key! You'll need it to start the application.");
        }

        String dbKey = prompt(in, "Database key (e.g., tenant1)");
        String dbName = prompt(in, "Database name (e.g., tenant1)");
        String jdbcUrl = prompt(in, "JDBC URL (e.g., jdbc:postgresql://localhost:5432/tenant1)");
        String dbUser = prompt(in, "Database username");
        String dbPassword = prompt(in, "Database password");
        String jwtSecret = prompt(in, "JWT secret");

        String encryptedPassword = encrypt(masterKey, dbPassword);
        String encryptedJwtSecret = encrypt(masterKey, jwtSecret);

        System.out.println("\n" + "=".repeat(60));
        System.out.println("✓ GENERATED SQL INSERT STATEMENT");
        System.out.println("=".repeat(60));
        System.out.println();
        System.out.println("-- Execute this SQL in postgrest_metadata database:");
        System.out.println();
        System.out.println("INSERT INTO postgrest_metadata.database_configs");
        System.out.println("(db_key, db_name, jdbc_url, db_user, db_password_encrypted,");
        System.out.println(" db_schemas, db_anon_role, jwt_secret_encrypted, pool_size, enabled)");
        System.out.println("VALUES");
        System.out.printf("('%s', '%s',\n", dbKey, dbName);
        System.out.printf(" '%s',\n", jdbcUrl);
        System.out.printf(" '%s',\n", dbUser);
        System.out.printf(" '%s',\n", encryptedPassword);
        System.out.println(" ARRAY['public'],");
        System.out.println(" 'web_anon',");
        System.out.printf(" '%s',\n", encryptedJwtSecret);
        System.out.println(" 10,");
        System.out.println(" true);");
        System.out.println();
        System.out.println("-- Set this environment variable before starting the app:");
        System.out.println("export PGRST_ENCRYPTION_MASTER_KEY=\"" + masterKey + "\"");
        System.out.println();
    }

    /**
     * Prompt helper — print a label, read one line from stdin. Returns "" on EOF / error
     * so an automated pipeline can supply values via heredoc without crashing.
     */
    private static String prompt(java.io.BufferedReader in, String label) throws java.io.IOException {
        System.out.print("\n" + label + ": ");
        System.out.flush();
        String line = in.readLine();
        return line == null ? "" : line.trim();
    }

    /**
     * CLI entry point. Subcommands:
     *   (no args)                  — interactive: build a full INSERT for database_configs
     *   generate-key               — print a fresh base64 256-bit master key
     *   encrypt <masterKey> <text> — encrypt one value
     *   decrypt <masterKey> <text> — decrypt one value
     *
     * <p>No values are hardcoded. The master key is read from args or env
     * ({@code PGRST_ENCRYPTION_MASTER_KEY}) — never baked into the source.
     */
    public static void main(String[] args) throws Exception {
        if (args.length == 0) {
            generateConfigInteractive();
            return;
        }
        switch (args[0]) {
            case "generate-key" -> {
                String key = generateMasterKey();
                System.out.println("Generated master encryption key:");
                System.out.println(key);
                System.out.println();
                System.out.println("Set as environment variable:");
                System.out.println("export PGRST_ENCRYPTION_MASTER_KEY=\"" + key + "\"");
            }
            case "encrypt" -> {
                if (args.length < 3) {
                    System.err.println("Usage: encrypt <master_key> <plaintext>");
                    System.exit(1);
                }
                System.out.println(encrypt(args[1], args[2]));
            }
            case "decrypt" -> {
                if (args.length < 3) {
                    System.err.println("Usage: decrypt <master_key> <ciphertext>");
                    System.exit(1);
                }
                System.out.println(decrypt(args[1], args[2]));
            }
            default -> {
                System.err.println("Unknown command: " + args[0]);
                System.err.println();
                System.err.println("Available commands:");
                System.err.println("  (no args)                 — interactive INSERT builder");
                System.err.println("  generate-key              — print a new master key");
                System.err.println("  encrypt <key> <text>      — encrypt text");
                System.err.println("  decrypt <key> <text>      — decrypt text");
                System.exit(1);
            }
        }
    }
}
