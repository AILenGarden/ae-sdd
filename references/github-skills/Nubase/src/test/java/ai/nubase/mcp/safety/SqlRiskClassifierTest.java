package ai.nubase.mcp.safety;

import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

class SqlRiskClassifierTest {

    private final SqlRiskClassifier classifier = new SqlRiskClassifier();

    @Test
    void classifiesReadQueries() {
        assertThat(classifier.classify("select * from todos")).isEqualTo(SqlRisk.READ);
        assertThat(classifier.classify("with recent as (select * from todos) select * from recent"))
                .isEqualTo(SqlRisk.READ);
    }

    @Test
    void classifiesSchemaWrites() {
        assertThat(classifier.classify("create table todos (id bigserial primary key)"))
                .isEqualTo(SqlRisk.SCHEMA_WRITE);
        assertThat(classifier.classify("alter table todos add column done boolean"))
                .isEqualTo(SqlRisk.SCHEMA_WRITE);
    }

    @Test
    void classifiesDataWrites() {
        assertThat(classifier.classify("insert into todos(text) values ('ship')"))
                .isEqualTo(SqlRisk.DATA_WRITE);
        assertThat(classifier.classify("update todos set done = true"))
                .isEqualTo(SqlRisk.DATA_WRITE);
        assertThat(classifier.classify("delete from todos where id = 1"))
                .isEqualTo(SqlRisk.DATA_WRITE);
    }

    @Test
    void classifiesDangerousStatements() {
        assertThat(classifier.classify("drop table todos")).isEqualTo(SqlRisk.DANGEROUS);
        assertThat(classifier.classify("truncate table todos")).isEqualTo(SqlRisk.DANGEROUS);
        assertThat(classifier.classify("delete from todos")).isEqualTo(SqlRisk.DANGEROUS);
    }

    @Test
    void mixedStatementsReturnHighestRisk() {
        assertThat(classifier.classify("select * from todos; drop table todos;"))
                .isEqualTo(SqlRisk.DANGEROUS);
        assertThat(classifier.classify("select * from todos; create table notes(id bigint);"))
                .isEqualTo(SqlRisk.SCHEMA_WRITE);
    }

    @Test
    void countsNonBlankStatements() {
        assertThat(classifier.countStatements("select 1; ; select 2;")).isEqualTo(2);
        assertThat(classifier.countStatements(null)).isZero();
    }

    @Test
    void blankSqlIsUnknown() {
        assertThat(classifier.classify(" ")).isEqualTo(SqlRisk.UNKNOWN);
    }
}
