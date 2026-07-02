package ai.nubase.auth.dto.storage;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class FileListRequest {

    private String bucketId;
    private String prefix;
    private Integer limit;
    private Integer offset;
    private String sortColumn;
    private String sortOrder;
    private String search;
}
