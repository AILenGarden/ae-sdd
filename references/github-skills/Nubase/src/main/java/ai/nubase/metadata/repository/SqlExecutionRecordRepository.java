package ai.nubase.metadata.repository;

import ai.nubase.metadata.entity.SqlExecutionRecord;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;

@Repository
public interface SqlExecutionRecordRepository extends JpaRepository<SqlExecutionRecord, Long> {

    List<SqlExecutionRecord> findByDatabaseKeyOrderByCreatedAtDesc(String databaseKey, Pageable pageable);
}
