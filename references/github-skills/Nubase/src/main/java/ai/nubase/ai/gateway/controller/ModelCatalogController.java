package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.entity.ModelPricing;
import ai.nubase.ai.gateway.repository.ModelPricingRepository;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.math.BigDecimal;
import java.math.RoundingMode;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 公开的模型目录 (无需认证), 由 marketing /models 页消费。
 */
@RestController
@RequestMapping("/api/v1/models")
@RequiredArgsConstructor
public class ModelCatalogController {

    private final ModelPricingRepository modelPricingRepository;

    // model_pricing 列名带 _cny 但 currency 字段是真实币种; 前端只展示 USD,
    // 当行是 CNY 时除以 7.2 折算 (临时, 等加 USD 列后改).
    private static final BigDecimal CNY_TO_USD = new BigDecimal("7.2");

    @GetMapping("/public")
    public ResponseEntity<Map<String, Object>> publicModels() {
        List<Map<String, Object>> data = new ArrayList<>();
        for (ModelPricing m : modelPricingRepository.findByIsActiveTrueOrderBySortOrderAscProviderAscModelAsc()) {
            Map<String, Object> dto = new LinkedHashMap<>();
            dto.put("slug", m.getModel());
            dto.put("name", m.getDisplayName() == null ? m.getModel() : m.getDisplayName());
            dto.put("provider", capitalize(m.getProvider()));
            dto.put("family", "general");
            dto.put("context", 0);
            dto.put("inputPrice", toUsd(m.getInputPricePer1MCny(), m.getCurrency()));
            dto.put("outputPrice", toUsd(m.getOutputPricePer1MCny(), m.getCurrency()));
            dto.put("cacheCreationPrice", toUsd(m.getCacheCreationPricePer1MCny(), m.getCurrency()));
            dto.put("cacheReadPrice", toUsd(m.getCacheReadPricePer1MCny(), m.getCurrency()));
            dto.put("description", m.getNotes() == null ? "" : m.getNotes());
            dto.put("tags", List.of());
            dto.put("quickstartExample", m.getQuickstartExample());
            dto.put("sortOrder", m.getSortOrder());
            data.add(dto);
        }
        return ResponseEntity.ok(Map.of("data", data));
    }

    private static BigDecimal toUsd(BigDecimal price, String currency) {
        if (price == null) return BigDecimal.ZERO;
        if ("USD".equalsIgnoreCase(currency)) return price.setScale(4, RoundingMode.HALF_UP);
        return price.divide(CNY_TO_USD, 4, RoundingMode.HALF_UP);
    }

    private static String capitalize(String s) {
        if (s == null || s.isEmpty()) return s;
        return Character.toUpperCase(s.charAt(0)) + s.substring(1);
    }
}
