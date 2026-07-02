package ai.nubase.auth.repository;

import ai.nubase.auth.entity.User;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface UserRepository extends JpaRepository<User, UUID> {

    Optional<User> findByEmail(String email);

    Optional<User> findByPhone(String phone);

    Optional<User> findByConfirmationToken(String confirmationToken);

    Optional<User> findByRecoveryToken(String recoveryToken);

    Optional<User> findByEmailChangeTokenNew(String emailChangeTokenNew);

    Optional<User> findByEmailChangeTokenCurrent(String emailChangeTokenCurrent);

    Optional<User> findByReauthenticationToken(String reauthenticationToken);

    boolean existsByEmail(String email);

    boolean existsByPhone(String phone);

    @Query("SELECT u FROM User u WHERE " +
            "LOWER(u.email) LIKE LOWER(CONCAT('%', :keyword, '%')) OR " +
            "LOWER(u.phone) LIKE LOWER(CONCAT('%', :keyword, '%')) OR " +
            "LOWER(u.role) LIKE LOWER(CONCAT('%', :keyword, '%')) OR " +
            "CAST(u.id AS string) LIKE CONCAT('%', :keyword, '%')")
    Page<User> searchByKeyword(@Param("keyword") String keyword, Pageable pageable);
}
