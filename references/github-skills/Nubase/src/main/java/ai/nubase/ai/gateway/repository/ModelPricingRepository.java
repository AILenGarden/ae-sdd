package ai.nubase.ai.gateway.repository;

import ai.nubase.ai.gateway.entity.ModelPricing;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.List;

public interface ModelPricingRepository extends JpaRepository<ModelPricing, Long> {
    List<ModelPricing> findByIsActiveTrueOrderBySortOrderAscProviderAscModelAsc();

    java.util.Optional<ModelPricing> findFirstByModelAndIsActiveTrue(String model);
}
