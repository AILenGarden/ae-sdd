package ai.nubase.postgrest.schema;

import lombok.Builder;
import lombok.Data;
import lombok.Singular;

import java.util.List;

/**
 * Represents a foreign key relationship
 */
@Data
@Builder
public class ForeignKey {
  private String constraintName;
  private String sourceSchema;
  private String sourceTable;
  @Singular
  private List<String> sourceColumns;
  private String targetSchema;
  private String targetTable;
  @Singular
  private List<String> targetColumns;
}
