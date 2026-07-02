package ai.nubase.auth.repository;

import ai.nubase.auth.entity.AuditLogEntry;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.UUID;

@Repository
public interface AuditLogEntryRepository extends JpaRepository<AuditLogEntry, UUID> {
}
