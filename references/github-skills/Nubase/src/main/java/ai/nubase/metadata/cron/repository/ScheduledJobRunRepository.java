package ai.nubase.metadata.cron.repository;

import ai.nubase.metadata.cron.entity.ScheduledJobRun;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

import java.time.Instant;
import java.util.List;
import java.util.UUID;

public interface ScheduledJobRunRepository extends JpaRepository<ScheduledJobRun, UUID> {

    List<ScheduledJobRun> findByJobIdOrderByStartedAtDesc(UUID jobId, Pageable pageable);

    List<ScheduledJobRun> findByProjectRefOrderByStartedAtDesc(String projectRef, Pageable pageable);

    // Bulk delete for retention pruning (derived deletes load every entity first).
    @Modifying
    @Query("delete from ScheduledJobRun r where r.startedAt < :cutoff")
    int deleteByStartedAtBefore(@Param("cutoff") Instant cutoff);
}
