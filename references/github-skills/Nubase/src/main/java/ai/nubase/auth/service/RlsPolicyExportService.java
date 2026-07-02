package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.admin.ExportRlsPoliciesRequest;
import ai.nubase.auth.dto.response.admin.ExportRlsPoliciesResponse;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;
import org.springframework.util.StringUtils;

import java.util.*;
import java.util.stream.Collectors;

/**
 * Service for exporting PostgreSQL RLS (Row Level Security) policies
 * <p>
 * Generates complete RLS policy statements including:
 * - ALTER TABLE ... ENABLE ROW LEVEL SECURITY
 * - CREATE POLICY statements with USING and WITH CHECK clauses
 * - Role assignments
 *
 * @author nubase
 * @since 2025-01-03
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class RlsPolicyExportService {

    private final JdbcTemplate jdbcTemplate;

    /**
     * Export RLS policies for tables in the database
     */
    public ExportRlsPoliciesResponse exportRlsPolicies(ExportRlsPoliciesRequest request) {
        long startTime = System.currentTimeMillis();

        try {
            String schemaName = request.getSchemaName();
            Set<String> tableFilter = parseTableFilter(request.getTableNames());
            boolean includeDropStatements = Boolean.TRUE.equals(request.getIncludeDropStatements());
            boolean groupBySchema = request.getGroupBySchema() == null || request.getGroupBySchema();

            // Query all RLS policies
            List<PolicyData> policies = queryRlsPolicies(schemaName, tableFilter);

            if (policies.isEmpty()) {
                String message = schemaName != null
                        ? "No RLS policies found in schema: " + schemaName
                        : "No RLS policies found in database";
                return ExportRlsPoliciesResponse.builder()
                        .success(true)
                        .schemaName(schemaName != null ? schemaName : "all")
                        .rlsPolicySql("")
                        .tableCount(0)
                        .policyCount(0)
                        .executionTimeMs(System.currentTimeMillis() - startTime)
                        .build();
            }

            // Generate RLS SQL
            if (groupBySchema && schemaName == null) {
                // Group by schema
                Map<String, String> rlsPoliciesBySchema = generateRlsSqlBySchema(policies, includeDropStatements);
                List<ExportRlsPoliciesResponse.RlsTableInfo> tablesWithRls = collectTableInfo(policies);

                return ExportRlsPoliciesResponse.builder()
                        .success(true)
                        .schemaName("all")
                        .rlsPoliciesBySchema(rlsPoliciesBySchema)
                        .tablesWithRls(tablesWithRls)
                        .tableCount(tablesWithRls.size())
                        .policyCount(policies.size())
                        .executionTimeMs(System.currentTimeMillis() - startTime)
                        .build();
            } else {
                // Single schema or ungrouped
                String rlsPolicySql = generateRlsSql(policies, includeDropStatements);
                List<ExportRlsPoliciesResponse.RlsTableInfo> tablesWithRls = collectTableInfo(policies);

                return ExportRlsPoliciesResponse.builder()
                        .success(true)
                        .schemaName(schemaName != null ? schemaName : "all")
                        .rlsPolicySql(rlsPolicySql)
                        .tablesWithRls(tablesWithRls)
                        .tableCount(tablesWithRls.size())
                        .policyCount(policies.size())
                        .executionTimeMs(System.currentTimeMillis() - startTime)
                        .build();
            }

        } catch (Exception e) {
            log.error("Failed to export RLS policies: {}", e.getMessage(), e);
            return ExportRlsPoliciesResponse.builder()
                    .success(false)
                    .schemaName(request.getSchemaName() != null ? request.getSchemaName() : "all")
                    .error("Failed to export RLS policies: " + e.getMessage())
                    .errorDetails(e.toString())
                    .executionTimeMs(System.currentTimeMillis() - startTime)
                    .build();
        }
    }

    /**
     * Parse comma-separated table names into a set
     */
    private Set<String> parseTableFilter(String tableNames) {
        if (tableNames == null || tableNames.strip().isEmpty()) {
            return Collections.emptySet();
        }
        return Arrays.stream(tableNames.split(","))
                .map(String::strip)
                .filter(s -> !s.isEmpty())
                .collect(Collectors.toSet());
    }

    /**
     * Query all RLS policies from pg_policies
     */
    private List<PolicyData> queryRlsPolicies(String schemaName, Set<String> tableFilter) {
        StringBuilder sql = new StringBuilder();
        sql.append("SELECT schemaname, tablename, policyname, cmd, roles, qual, with_check ");
        sql.append("FROM pg_policies ");

        List<Object> params = new ArrayList<>();
        List<String> conditions = new ArrayList<>();

        if (schemaName != null && !schemaName.strip().isEmpty()) {
            conditions.add("schemaname = ?");
            params.add(schemaName);
        }

        // Exclude system schemas
        conditions.add("schemaname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')");

        if (!conditions.isEmpty()) {
            sql.append("WHERE ").append(String.join(" AND ", conditions)).append(" ");
        }

        sql.append("ORDER BY schemaname, tablename, policyname");

        List<PolicyData> policies = jdbcTemplate.query(
                sql.toString(),
                (rs, rowNum) -> {
                    PolicyData policy = new PolicyData();
                    policy.schemaname = rs.getString("schemaname");
                    policy.tablename = rs.getString("tablename");
                    policy.policyname = rs.getString("policyname");
                    policy.cmd = rs.getString("cmd");
                    policy.roles = rs.getString("roles");
                    policy.qual = rs.getString("qual");
                    policy.withCheck = rs.getString("with_check");
                    return policy;
                },
                params.toArray()
        );

        // Apply table filter if provided
        if (!tableFilter.isEmpty()) {
            policies = policies.stream()
                    .filter(p -> tableFilter.contains(p.tablename))
                    .collect(Collectors.toList());
        }

        return policies;
    }

    /**
     * Generate RLS SQL for all policies
     */
    private String generateRlsSql(List<PolicyData> policies, boolean includeDropStatements) {
        if (policies.isEmpty()) {
            return "";
        }

        StringBuilder rlsPolicyBuilder = new StringBuilder();

        // Group policies by table to avoid duplicate ALTER TABLE statements
        Map<String, List<PolicyData>> policiesByTable = policies.stream()
                .collect(Collectors.groupingBy(p -> p.schemaname + "." + p.tablename, LinkedHashMap::new, Collectors.toList()));

        // Generate ALTER TABLE statements
        for (String tableKey : policiesByTable.keySet()) {
            PolicyData firstPolicy = policiesByTable.get(tableKey).get(0);
            rlsPolicyBuilder.append("ALTER TABLE ")
                    .append(firstPolicy.schemaname).append(".").append(firstPolicy.tablename)
                    .append(" ENABLE ROW LEVEL SECURITY;\n");
        }

        if (!policiesByTable.isEmpty()) {
            rlsPolicyBuilder.append("\n");
        }

        // Generate DROP POLICY statements if requested
        if (includeDropStatements) {
            for (PolicyData policy : policies) {
                rlsPolicyBuilder.append("DROP POLICY IF EXISTS ")
                        .append(policy.policyname)
                        .append(" ON ")
                        .append(policy.schemaname).append(".").append(policy.tablename)
                        .append(";\n");
            }
            rlsPolicyBuilder.append("\n");
        }

        // Generate CREATE POLICY statements
        for (PolicyData policy : policies) {
            rlsPolicyBuilder.append(generateCreatePolicyStatement(policy));
        }

        return rlsPolicyBuilder.toString();
    }

    /**
     * Generate RLS SQL grouped by schema
     */
    private Map<String, String> generateRlsSqlBySchema(List<PolicyData> policies, boolean includeDropStatements) {
        Map<String, List<PolicyData>> policiesBySchema = policies.stream()
                .collect(Collectors.groupingBy(p -> p.schemaname, LinkedHashMap::new, Collectors.toList()));

        Map<String, String> result = new LinkedHashMap<>();
        for (Map.Entry<String, List<PolicyData>> entry : policiesBySchema.entrySet()) {
            String schema = entry.getKey();
            List<PolicyData> schemaPolicies = entry.getValue();
            String sql = generateRlsSql(schemaPolicies, includeDropStatements);
            result.put(schema, sql);
        }

        return result;
    }

    /**
     * Generate CREATE POLICY statement for a single policy
     */
    private String generateCreatePolicyStatement(PolicyData policy) {
        StringBuilder stmt = new StringBuilder();

        stmt.append("CREATE POLICY ")
                .append(policy.policyname)
                .append(" ON ")
                .append(policy.schemaname).append(".").append(policy.tablename);

        // Add command type (ALL, SELECT, INSERT, UPDATE, DELETE)
        if (StringUtils.hasText(policy.cmd)) {
            stmt.append(" FOR ").append(policy.cmd);
        }

        // Handle roles field
        if (StringUtils.hasText(policy.roles)) {
            String cleanedRoles = policy.roles.replace("{", "").replace("}", "").strip();
            // Don't add TO clause if role is PUBLIC
            if (!"public".equalsIgnoreCase(cleanedRoles)) {
                stmt.append(" TO ").append(cleanedRoles);
            }
        }

        // USING clause
        if (StringUtils.hasText(policy.qual)) {
            stmt.append("\n    USING (").append(policy.qual).append(")");
        }

        // WITH CHECK clause (only add if different from USING)
        if (StringUtils.hasText(policy.withCheck) && !policy.withCheck.equals(policy.qual)) {
            stmt.append("\n    WITH CHECK (").append(policy.withCheck).append(")");
        }

        stmt.append(";\n\n");

        return stmt.toString();
    }

    /**
     * Collect table information with RLS enabled
     */
    private List<ExportRlsPoliciesResponse.RlsTableInfo> collectTableInfo(List<PolicyData> policies) {
        Map<String, List<PolicyData>> policiesByTable = policies.stream()
                .collect(Collectors.groupingBy(p -> p.schemaname + "." + p.tablename, LinkedHashMap::new, Collectors.toList()));

        List<ExportRlsPoliciesResponse.RlsTableInfo> tablesWithRls = new ArrayList<>();

        for (Map.Entry<String, List<PolicyData>> entry : policiesByTable.entrySet()) {
            List<PolicyData> tablePolicies = entry.getValue();
            PolicyData firstPolicy = tablePolicies.get(0);

            // Query RLS status from pg_class
            Boolean[] rlsStatus = getRlsStatus(firstPolicy.schemaname, firstPolicy.tablename);

            ExportRlsPoliciesResponse.RlsTableInfo tableInfo = ExportRlsPoliciesResponse.RlsTableInfo.builder()
                    .schemaName(firstPolicy.schemaname)
                    .tableName(firstPolicy.tablename)
                    .policyCount(tablePolicies.size())
                    .rlsEnabled(rlsStatus[0])
                    .rlsForced(rlsStatus[1])
                    .policyNames(tablePolicies.stream()
                            .map(p -> p.policyname)
                            .collect(Collectors.toList()))
                    .build();

            tablesWithRls.add(tableInfo);
        }

        return tablesWithRls;
    }

    /**
     * Get RLS status for a table
     * Returns [rlsEnabled, rlsForced]
     */
    private Boolean[] getRlsStatus(String schemaName, String tableName) {
        String sql = """
                SELECT
                    c.relrowsecurity as rls_enabled,
                    c.relforcerowsecurity as rls_forced
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = ? AND c.relname = ?
                """;

        try {
            return jdbcTemplate.queryForObject(sql, (rs, rowNum) -> {
                return new Boolean[]{
                        rs.getBoolean("rls_enabled"),
                        rs.getBoolean("rls_forced")
                };
            }, schemaName, tableName);
        } catch (Exception e) {
            log.warn("Failed to get RLS status for {}.{}: {}", schemaName, tableName, e.getMessage());
            return new Boolean[]{true, false}; // Default values
        }
    }

    /**
     * Policy data holder
     */
    private static class PolicyData {
        String schemaname;
        String tablename;
        String policyname;
        String cmd;
        String roles;
        String qual;
        String withCheck;
    }
}
