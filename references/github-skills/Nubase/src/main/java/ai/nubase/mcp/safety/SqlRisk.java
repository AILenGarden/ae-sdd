package ai.nubase.mcp.safety;

public enum SqlRisk {
    UNKNOWN,
    READ,
    DATA_WRITE,
    SCHEMA_WRITE,
    DANGEROUS
}
