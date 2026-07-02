package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.entity.ModelPricing;
import ai.nubase.ai.gateway.repository.ModelPricingRepository;
import ai.nubase.auth.annotation.RequireServiceRole;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

/**
 * 项目级模型定价管理（控制面）。
 * <p>作用于当前项目租户库的 {@code ai_gateway.model_pricing}，供计算每日 token 成本使用（仅统计，不计费）。
 */
@Slf4j
@RestController
@RequestMapping("/ai-gateway/admin/v1/pricing")
@RequiredArgsConstructor
@RequireServiceRole
public class ModelPricingController {

    private final ModelPricingRepository modelPricingRepository;

    @GetMapping
    public ResponseEntity<List<ModelPricing>> list() {
        return ResponseEntity.ok(
                modelPricingRepository.findByIsActiveTrueOrderBySortOrderAscProviderAscModelAsc());
    }

    @GetMapping("/all")
    public ResponseEntity<List<ModelPricing>> listAll() {
        return ResponseEntity.ok(modelPricingRepository.findAll());
    }

    @PostMapping
    public ResponseEntity<ModelPricing> create(@RequestBody ModelPricing body) {
        body.setId(null);
        return ResponseEntity.ok(modelPricingRepository.save(body));
    }

    @PutMapping("/{id}")
    public ResponseEntity<?> update(@PathVariable Long id, @RequestBody ModelPricing body) {
        return modelPricingRepository.findById(id)
                .map(existing -> {
                    body.setId(id);
                    body.setCreatedAt(existing.getCreatedAt());
                    return ResponseEntity.ok(modelPricingRepository.save(body));
                })
                .orElseGet(() -> ResponseEntity.notFound().build());
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Map<String, Object>> delete(@PathVariable Long id) {
        modelPricingRepository.deleteById(id);
        return ResponseEntity.ok(Map.of("ok", true));
    }
}
