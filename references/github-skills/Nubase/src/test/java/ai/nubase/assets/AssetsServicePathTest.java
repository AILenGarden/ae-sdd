package ai.nubase.assets;

import ai.nubase.assets.service.AssetsExceptions.AssetsException;
import ai.nubase.assets.service.AssetsService;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class AssetsServicePathTest {

    @Test
    void normalizesLeadingAndTrailingSlashes() {
        assertThat(AssetsService.normalizePath("/img/logo.png")).isEqualTo("img/logo.png");
        assertThat(AssetsService.normalizePath("img/logo.png/")).isEqualTo("img/logo.png");
        assertThat(AssetsService.normalizePath("  app.css ")).isEqualTo("app.css");
    }

    @Test
    void acceptsTypicalStaticAssetPaths() {
        assertThat(AssetsService.normalizePath("js/main.bundle-v1.2.js")).isEqualTo("js/main.bundle-v1.2.js");
        assertThat(AssetsService.normalizePath("fonts/Inter_Regular.woff2")).isEqualTo("fonts/Inter_Regular.woff2");
    }

    @Test
    void rejectsTraversalAndEmptySegments() {
        assertThatThrownBy(() -> AssetsService.normalizePath("../secrets")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("img/../../x")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("img//logo.png")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("img/./logo.png")).isInstanceOf(AssetsException.class);
    }

    @Test
    void rejectsUnsafeCharactersAndBlankPaths() {
        assertThatThrownBy(() -> AssetsService.normalizePath("a b.png")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("x%2e%2e/y")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("   ")).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath(null)).isInstanceOf(AssetsException.class);
        assertThatThrownBy(() -> AssetsService.normalizePath("a".repeat(1025))).isInstanceOf(AssetsException.class);
    }
}
