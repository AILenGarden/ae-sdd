//! Contract checks for the reference-backed DR, Story, and TestCase templates.
//!
//! These assertions intentionally read the authoritative source files directly.
//! The templates may grow, but the reference semantics and required ordering
//! must not silently disappear.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_source(relative: &str) -> String {
    let path = workspace_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read source {relative:?} at {}: {error}", path.display()))
}

fn first_semantic_heading_after_h1(text: &str) -> &str {
    let mut after_h1 = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("# ") {
            after_h1 = true;
            continue;
        }
        if after_h1
            && line.starts_with("## ")
            && !line.starts_with("### ")
            && !line.starts_with("#### ")
        {
            return line;
        }
    }
    panic!("template has no H2 after H1");
}

#[test]
fn all_templates_put_metadata_first_and_retain_reference_fields() {
    let dr = read_source("source/templates/design/dr-template.md");
    let story = read_source("source/templates/design/story-template.md");
    let testcase = read_source("source/templates/testcase/be-testcase-template.md");

    assert!(first_semantic_heading_after_h1(&dr).contains("元信息"));
    assert!(first_semantic_heading_after_h1(&story).contains("元信息"));
    assert!(first_semantic_heading_after_h1(&testcase).contains("元信息"));

    for marker in ["文档类型", "来源 PRD", "来源 DR", "作者"] {
        assert!(story.contains(marker), "Story metadata misses {marker}");
        assert!(
            testcase.contains(marker),
            "TestCase metadata misses {marker}"
        );
    }
    for marker in ["功能点 ID", "优先级", "Superseded"] {
        assert!(
            story.contains(marker),
            "Story baseline metadata misses {marker}"
        );
    }
    for marker in ["Automated", "Superseded"] {
        assert!(testcase.contains(marker), "TestCase status misses {marker}");
    }
}

#[test]
fn story_retains_reference_sections_and_interface_entry_forms() {
    let story = read_source("source/templates/design/story-template.md");
    for marker in [
        "## 业务价值",
        "数据前置",
        "权限前置",
        "系统前置",
        "父工程",
        "Kafka topic",
        "job-spring-boot-starter",
        "AC ID",
        "## 用例设计映射",
        "## 偏离声明",
        "基线 / 契约",
        "当前决策或用途",
        "原因 / SLA",
        "风险与降级",
        "批准 / 责任方",
        "## 第三方服务",
        "调用方式",
        "文档地址",
        "负责人/联系方式",
        "delete_flag",
        "last_updated_date",
        "bootstrap/application",
        "Nacos",
        "测试层级",
        "##### Request",
        "data / VO 字段",
        "上游依赖",
        "下游影响",
        "缓解方式",
        "Task / 任务",
        "说明",
        "涉及工程 / 层",
        "状态",
        "icec-cloud-life-im-bff",
        "icec-cloud-life-im",
        "icec-cloud-life-user-spi",
        "icec-cloud-life-user-api",
        "icec-cloud-boss-user-api",
    ] {
        assert!(
            story.contains(marker),
            "Story misses reference marker {marker}"
        );
    }
}

#[test]
fn story_separates_new_and_existing_interface_contracts() {
    let story = read_source("source/templates/design/story-template.md");
    assert_eq!(
        story.lines().filter(|line| *line == "## 接口契约").count(),
        1,
        "Story must expose one interface-contract section"
    );
    assert!(
        !story.contains("## 接口契约-SPI") && !story.contains("## 接口契约-REST"),
        "protocol must not be the first grouping dimension"
    );

    let directory_start = story.find("**接口目录**").expect("interface directory");
    let directory_end = story[directory_start..]
        .find("### 核心设计 · 分析")
        .map(|offset| directory_start + offset)
        .expect("interface directory boundary");
    let directory = &story[directory_start..directory_end];
    let directory_new = directory
        .find("**新增接口**")
        .expect("new interface directory");
    let directory_existing = directory
        .find("**复用 / 既有接口**")
        .expect("existing interface directory");
    assert!(directory_new < directory_existing);
    assert_eq!(
        directory.matches("| 类型 | 编号 | 接口 / 签名 |").count(),
        2,
        "interface groups need separate directory tables"
    );
    assert!(
        !directory.contains("| 分组 |"),
        "interface directory must not collapse groups into one table"
    );

    let start = story.find("## 接口契约").expect("interface contract");
    let end = story[start..]
        .find("<a id=\"state-transition-overview\"></a>")
        .map(|offset| start + offset)
        .expect("interface contract boundary");
    let contract = &story[start..end];
    let new_interfaces = contract.find("### 新增接口").expect("new interface group");
    let existing_interfaces = contract
        .find("### 复用 / 既有接口")
        .expect("existing interface group");
    assert!(
        new_interfaces < existing_interfaces,
        "new interfaces must be listed before existing interfaces"
    );
    for detail in ["### REST-1", "### SPI-1"] {
        let position = contract
            .find(detail)
            .unwrap_or_else(|| panic!("interface contract misses {detail}"));
        assert!(
            new_interfaces < position && position < existing_interfaces,
            "{detail} must live inside the new-interface group"
        );
    }

    for marker in [
        "直接复用",
        "既有扩展",
        "回归验证",
        "现状证据",
        "本次变更边界",
        "每个接口只出现一次",
    ] {
        assert!(
            contract.contains(marker),
            "interface grouping misses {marker}"
        );
    }
    for compatibility_anchor in ["interface-contract-spi", "interface-contract-rest"] {
        assert!(
            contract.contains(&format!("<a id=\"{compatibility_anchor}\"></a>")),
            "interface contract misses compatibility anchor {compatibility_anchor}"
        );
    }
}

#[test]
fn story_interface_contract_consumers_follow_the_same_grouping() {
    for path in [
        "source/standards/templates/template-layout-standard.md",
        "source/standards/story/story-frontend-contract-standard.md",
        "source/templates/design/be-story-review-logic-summary-template.md",
    ] {
        let text = read_source(path);
        assert!(
            text.contains("新增接口"),
            "{path} misses the new-interface group"
        );
        assert!(
            text.contains("复用 / 既有接口") || text.contains("复用与既有接口"),
            "{path} misses the existing-interface group"
        );
        assert!(
            !text.contains("## 接口契约-SPI") && !text.contains("## 接口契约-REST"),
            "{path} still documents protocol-first Story sections"
        );
    }

    let review = read_source("source/templates/design/be-story-review-logic-summary-template.md");
    let new_interfaces = review
        .find("### 6.1 新增接口")
        .expect("review new-interface section");
    let existing_interfaces = review
        .find("### 6.2 复用 / 既有接口")
        .expect("review existing-interface section");
    assert!(new_interfaces < existing_interfaces);
    assert!(
        !review.contains("| 分组 |"),
        "review summary must use separate interface tables"
    );
    assert!(review[existing_interfaces..].contains("现状证据"));
}

#[test]
fn every_rest_block_has_the_eight_required_modules_in_order() {
    let story = read_source("source/templates/design/story-template.md");
    let start = story.find("### REST-1").expect("REST-1 block");
    let end = story[start..]
        .find("<a id=\"spi-1\"></a>")
        .map(|offset| start + offset)
        .expect("REST copy boundary");
    let block = &story[start..end];
    let modules = [
        "#### 1. 基本信息",
        "#### 2. 鉴权安全",
        "#### 3. 请求定义",
        "#### 4. 响应定义",
        "#### 5. 错误码表",
        "#### 6. 非功能",
        "#### 7. 调用示例",
        "#### 8. 版本变更",
    ];
    let mut cursor = 0;
    for module in modules {
        let position = block[cursor..]
            .find(module)
            .unwrap_or_else(|| panic!("REST block misses {module}"));
        cursor += position + module.len();
    }
    for marker in [
        "名称",
        "描述",
        "Method / Path",
        "版本",
        "维护人",
        "Token 方式",
        "参数位置",
        "权限要求",
        "Path 参数",
        "Query 参数",
        "Header 参数",
        "Body 参数",
        "业务码",
        "| code | integer |",
        "message",
        "data",
        "traceId",
        "分页",
        "处理建议",
        "幂等",
        "限流",
        "超时",
        "重试",
        "cURL",
        "SDK",
        "CHANGELOG/",
        "PagedModels<T>",
    ] {
        assert!(block.contains(marker), "REST block misses {marker}");
    }
    assert!(
        !block.contains("businessCode"),
        "REST response must use the project envelope key code"
    );
    assert!(
        !block.contains("历史记录"),
        "REST body must not contain history"
    );
    assert!(
        !block.contains("基线兼容") && !block.contains("兼容视图") && !block.contains("（保留）"),
        "REST block must expose one semantic filling source per field"
    );
    let version_change = block
        .split_once("#### 8. 版本变更")
        .map(|(_, content)| content.trim())
        .expect("REST version-change module");
    assert_eq!(
        version_change
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>(),
        [
            "正文不写变更历史；统一引用 [`CHANGELOG/`](../../CHANGELOG/)。",
            "---"
        ],
        "REST version-change module may only reference CHANGELOG/"
    );
}

#[test]
fn dr_retains_all_reference_sections() {
    let dr = read_source("source/templates/design/dr-template.md");
    let reference_sections = [
        "元信息",
        "设计目标",
        "约束承接",
        "关键决策",
        "架构概览",
        "关键时序",
        "数据模型",
        "接口契约",
        "状态与业务规则",
        "故障模式",
        "权限、安全与审计",
        "Story 拆分",
        "测试策略",
        "可观测性",
        "发布、迁移与回滚",
        "风险",
        "未决问题",
        "追踪",
    ];
    let mut cursor = 0;
    for (number, section) in reference_sections.iter().enumerate() {
        let heading = format!("## {}. {section}", number + 1);
        let position = dr[cursor..]
            .find(&heading)
            .unwrap_or_else(|| panic!("DR misses reference section {heading}"));
        cursor += position + heading.len();
    }
}

#[test]
fn spi_block_remains_protocol_neutral() {
    let story = read_source("source/templates/design/story-template.md");
    let start = story.find("### SPI-1").expect("SPI-1 block");
    let end = story[start..]
        .find("### 复用 / 既有接口")
        .map(|offset| start + offset)
        .expect("SPI block boundary");
    let block = &story[start..end];

    for required in [
        "请求模型",
        "返回模型",
        "Request",
        "DTO",
        "方向",
        "必需性",
        "约束",
        "业务语义",
        "说明",
        "超时 / 重试",
        "幂等",
        "错误与降级",
    ] {
        assert!(block.contains(required), "SPI block misses {required}");
    }
    for rest_only in ["Token 方式", "HTTP 状态码", "pagination", "traceId", "cURL"] {
        assert!(
            !block.contains(rest_only),
            "SPI block must not include REST-only field {rest_only}"
        );
    }
}

#[test]
fn testcase_retains_reference_examples_and_execution_report_contract() {
    let testcase = read_source("source/templates/testcase/be-testcase-template.md");
    for marker in [
        "TC-001",
        "TC-002",
        "Controller集成",
        "JUnit + Mockito",
        "Spring Boot Test + 开发库",
        "@Transactional",
        "@Rollback",
        "testing.md",
        "执行与报告要求",
        "Story ID / AC ID / 用例 ID",
        "UserControllerTest#updateUserStatus_success",
        "UserAppServiceTest#updateUserStatus_userNotFound",
        "无法自动化",
        "剩余风险",
        "期望行为 / 期望结果",
        "业务断言 / 断言",
        "Test double 配置 / Mock 配置",
    ] {
        assert!(testcase.contains(marker), "TestCase misses {marker}");
    }
    assert!(
        !testcase.contains("|---|"),
        "table separators must contain spaces"
    );
    assert!(
        !testcase.lines().any(|line| {
            line.starts_with('#')
                && (line.contains("`必填") || line.contains("`选填") || line.contains("`条件必填"))
        }),
        "TestCase filling obligations must live in the declaration table"
    );
    assert_eq!(
        testcase
            .matches("| AC ID | 风险/假设 ID | 用例 ID | 场景 |")
            .count(),
        1,
        "TestCase must have one authoritative coverage matrix"
    );
    assert!(
        !testcase.contains("基线覆盖矩阵兼容视图")
            && !testcase.contains("基线历史行")
            && !testcase.contains("| MockMvc |"),
        "TestCase must explain the MockMvc baseline without an active duplicate row"
    );
}

#[test]
fn story_section_markers_anchors_and_guide_entries_remain_one_to_one() {
    let story = read_source("source/templates/design/story-template.md");
    let guide = read_source("source/standards/story/story-writing-guide.md");
    let h2_count = story.lines().filter(|line| line.starts_with("## ")).count();
    let marker_prefix = "<!-- ae-sdd:story-section id=";
    let guide_prefix = "<!-- ae-sdd:story-guide section-id=";
    let mut template_ids = BTreeSet::new();
    let mut guide_ids = BTreeSet::new();
    let mut template_order = Vec::new();
    let mut guide_order = Vec::new();

    for line in story.lines() {
        if let Some(rest) = line.strip_prefix(marker_prefix) {
            let id = rest
                .split_once(" layer=")
                .map(|(id, _)| id)
                .expect("story marker layer");
            assert!(template_ids.insert(id), "duplicate story section id {id}");
            template_order.push(id);
            assert!(story.contains(&format!("<a id=\"{id}\"></a>")));
        }
    }
    for line in guide.lines() {
        if let Some(rest) = line.strip_prefix(guide_prefix) {
            let id = rest.strip_suffix(" -->").expect("guide marker suffix");
            assert!(guide_ids.insert(id), "duplicate guide section id {id}");
            guide_order.push(id);
        }
    }

    assert_eq!(
        template_ids.len(),
        h2_count,
        "every Story H2 needs one marker"
    );
    assert_eq!(
        template_ids, guide_ids,
        "Story template and guide ids drifted"
    );
    assert_eq!(
        template_order, guide_order,
        "Story template and guide section order drifted"
    );
}
