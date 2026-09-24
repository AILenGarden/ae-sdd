use std::ops::ControlFlow;

use sqlparser::ast::{Expr, Query, SelectFlavor, SetExpr, Statement, TableFactor, Visit, Visitor};
use sqlparser::dialect::{Dialect, MySqlDialect, PostgreSqlDialect};
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer, Whitespace};
use thiserror::Error;

use crate::registry::Engine;
use crate::registry::WriteLevel;

#[derive(Debug, Error)]
pub enum SqlPolicyError {
    #[error("SQL statement must not be empty")]
    Empty,
    #[error("SQL statement contains a NUL byte")]
    Nul,
    #[error("SQL cannot be parsed safely: {0}")]
    Parse(String),
    #[error("exactly one SQL statement is required")]
    MultipleStatements,
    #[error("statement type {0} is not allowed")]
    StatementType(String),
    #[error("SQL construct {0} is not allowed")]
    Construct(String),
}

pub fn validate_read_only(sql: &str, engine: Engine) -> Result<String, SqlPolicyError> {
    let statement = sql.trim();
    if statement.is_empty() {
        return Err(SqlPolicyError::Empty);
    }
    if statement.contains('\0') {
        return Err(SqlPolicyError::Nul);
    }

    let (statements, tokens) = match engine {
        Engine::Mysql => parse_and_tokenize(&MySqlDialect {}, statement),
        Engine::Postgresql => parse_and_tokenize(&PostgreSqlDialect {}, statement),
    }?;

    if statements.len() != 1 {
        return Err(SqlPolicyError::MultipleStatements);
    }

    reject_blocked_tokens(&tokens)?;
    validate_ast(&statements[0], engine)?;

    Ok(statement.to_owned())
}

pub fn validate_for_level(
    sql: &str,
    engine: Engine,
    level: WriteLevel,
) -> Result<String, SqlPolicyError> {
    if level == WriteLevel::None {
        return validate_read_only(sql, engine);
    }
    let statement = sql.trim();
    if statement.is_empty() || statement.contains('\0') {
        return Err(SqlPolicyError::Empty);
    }
    let (statements, tokens) = match engine {
        Engine::Mysql => parse_and_tokenize(&MySqlDialect {}, statement),
        Engine::Postgresql => parse_and_tokenize(&PostgreSqlDialect {}, statement),
    }?;
    if statements.len() != 1 {
        return Err(SqlPolicyError::MultipleStatements);
    }
    let first = tokens
        .iter()
        .find_map(|token| match token {
            Token::Word(word) => Some(word.value.to_ascii_uppercase()),
            _ => None,
        })
        .unwrap_or_default();
    let dml = matches!(first.as_str(), "INSERT" | "UPDATE" | "DELETE");
    let normalized = statement.to_ascii_uppercase();
    let ddl = normalized.starts_with("CREATE TABLE ")
        || normalized.starts_with("CREATE INDEX ")
        || normalized.starts_with("ALTER TABLE ")
        || normalized.starts_with("DROP INDEX ");
    if !(dml || ddl) {
        // Not a statement this level can extend: treat it as a read, so reads stay
        // subject to the read-only policy and report the reason they were refused.
        return validate_read_only(statement, engine);
    }
    reject_blocked_tokens_for_write(&tokens, level)?;
    if !(dml || level.allows_ddl()) {
        return Err(SqlPolicyError::StatementType(first.to_ascii_lowercase()));
    }
    Ok(statement.to_owned())
}

fn reject_blocked_tokens_for_write(
    tokens: &[Token],
    level: WriteLevel,
) -> Result<(), SqlPolicyError> {
    let forbidden = [
        "GRANT",
        "REVOKE",
        "TRUNCATE",
        "DROP DATABASE",
        "CALL",
        "EXECUTE",
        "COPY",
        "LOAD",
        "PRAGMA",
        "TRANSACTION",
        "COMMIT",
        "ROLLBACK",
    ];
    if tokens
        .iter()
        .any(|t| matches!(t, Token::Assignment | Token::AtSign | Token::AtAt))
    {
        return Err(SqlPolicyError::Construct(
            "session or system variable".to_owned(),
        ));
    }
    if tokens.iter().any(|t| {
        matches!(
            t,
            Token::Whitespace(
                Whitespace::SingleLineComment { .. } | Whitespace::MultiLineComment(_)
            )
        )
    }) {
        return Err(SqlPolicyError::Construct("comment".to_owned()));
    }
    let text = tokens
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();
    if forbidden.iter().any(|item| text.contains(item)) {
        return Err(SqlPolicyError::Construct(
            "blocked write construct".to_owned(),
        ));
    }
    if !level.allows_dml() {
        return Err(SqlPolicyError::StatementType("dml".to_owned()));
    }
    Ok(())
}

fn validate_ast(statement: &Statement, engine: Engine) -> Result<(), SqlPolicyError> {
    match statement {
        Statement::Query(_) | Statement::ExplainTable { .. } => {}
        Statement::Explain {
            analyze,
            statement,
            options,
            ..
        } => {
            if *analyze || !safe_explain_options(options.as_deref().unwrap_or_default()) {
                return Err(SqlPolicyError::Construct("explain options".to_owned()));
            }
            if !matches!(statement.as_ref(), Statement::Query(_)) {
                return Err(SqlPolicyError::Construct(
                    "explain non-query statement".to_owned(),
                ));
            }
        }
        Statement::ShowColumns { .. }
        | Statement::ShowDatabases { .. }
        | Statement::ShowSchemas { .. }
        | Statement::ShowCharset(_)
        | Statement::ShowTables { .. }
        | Statement::ShowViews { .. }
        | Statement::ShowCollation { .. } => {}
        other => {
            return Err(SqlPolicyError::StatementType(
                other
                    .to_string()
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_ascii_lowercase(),
            ));
        }
    }

    let mut visitor = PolicyVisitor { engine };
    match statement.visit(&mut visitor) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(reason) => Err(SqlPolicyError::Construct(reason)),
    }
}

fn safe_explain_options(options: &[sqlparser::ast::UtilityOption]) -> bool {
    options.iter().all(|option| {
        matches!(
            option.name.value.to_ascii_lowercase().as_str(),
            "verbose" | "costs" | "format" | "settings" | "generic_plan"
        )
    })
}

struct PolicyVisitor {
    engine: Engine,
}

impl Visitor for PolicyVisitor {
    type Break = String;

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
        if !query.locks.is_empty() {
            return ControlFlow::Break("locking query".to_owned());
        }
        if query.for_clause.is_some()
            || query.settings.is_some()
            || query.format_clause.is_some()
            || !query.pipe_operators.is_empty()
        {
            return ControlFlow::Break("unsupported query clause".to_owned());
        }
        validate_set_expr(&query.body)
    }

    fn pre_visit_table_factor(&mut self, table: &TableFactor) -> ControlFlow<Self::Break> {
        match table {
            TableFactor::Table {
                args,
                with_hints,
                version,
                with_ordinality,
                partitions,
                json_path,
                sample,
                index_hints,
                ..
            } if args.is_none()
                && with_hints.is_empty()
                && version.is_none()
                && !with_ordinality
                && partitions.is_empty()
                && json_path.is_none()
                && sample.is_none()
                && index_hints.is_empty() =>
            {
                ControlFlow::Continue(())
            }
            TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => {
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Break("table-valued or unsupported table construct".to_owned()),
        }
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Self::Break> {
        if let Expr::Identifier(identifier) = expr
            && matches!(
                identifier.value.to_ascii_lowercase().as_str(),
                "current_role" | "current_user" | "session_user" | "system_user" | "user"
            )
        {
            return ControlFlow::Break(format!(
                "connection identity {}",
                identifier.value.to_ascii_lowercase()
            ));
        }
        if let Expr::Function(function) = expr {
            let rendered = function
                .name
                .to_string()
                .replace(['`', '"'], "")
                .to_ascii_lowercase();
            let parts: Vec<&str> = rendered.split('.').collect();
            let name = parts.last().copied().unwrap_or_default();
            let qualified_safely = parts.len() == 1
                || (self.engine == Engine::Postgresql
                    && parts.len() == 2
                    && parts[0] == "pg_catalog");
            if !qualified_safely || !is_safe_function(name, self.engine) {
                return ControlFlow::Break(format!("function {rendered}"));
            }
        }
        ControlFlow::Continue(())
    }
}

fn validate_set_expr(expression: &SetExpr) -> ControlFlow<String> {
    match expression {
        SetExpr::Select(select)
            if select.into.is_none()
                && select.exclude.is_none()
                && select.lateral_views.is_empty()
                && select.prewhere.is_none()
                && select.cluster_by.is_empty()
                && select.distribute_by.is_empty()
                && select.sort_by.is_empty()
                && select.qualify.is_none()
                && select.value_table_mode.is_none()
                && select.connect_by.is_none()
                && select.flavor == SelectFlavor::Standard =>
        {
            ControlFlow::Continue(())
        }
        SetExpr::Query(query) => validate_set_expr(&query.body),
        SetExpr::SetOperation { left, right, .. } => {
            validate_set_expr(left)?;
            validate_set_expr(right)
        }
        _ => ControlFlow::Break("non-select query body".to_owned()),
    }
}

fn is_safe_function(name: &str, engine: Engine) -> bool {
    const COMMON: &[&str] = &[
        "abs",
        "array_agg",
        "avg",
        "bit_and",
        "bit_or",
        "ceil",
        "ceiling",
        "char_length",
        "coalesce",
        "concat",
        "concat_ws",
        "count",
        "current_date",
        "current_time",
        "current_timestamp",
        "date_part",
        "date_trunc",
        "dense_rank",
        "first_value",
        "floor",
        "greatest",
        "group_concat",
        "json_agg",
        "json_array_length",
        "json_extract",
        "json_length",
        "json_type",
        "json_unquote",
        "jsonb_agg",
        "lag",
        "last_value",
        "lead",
        "least",
        "left",
        "length",
        "lower",
        "localtime",
        "localtimestamp",
        "lpad",
        "ltrim",
        "max",
        "md5",
        "min",
        "mod",
        "nth_value",
        "nullif",
        "octet_length",
        "position",
        "power",
        "rank",
        "repeat",
        "replace",
        "reverse",
        "right",
        "round",
        "row_number",
        "rpad",
        "rtrim",
        "sign",
        "sqrt",
        "stddev",
        "stddev_pop",
        "stddev_samp",
        "string_agg",
        "substr",
        "substring",
        "sum",
        "trim",
        "trunc",
        "upper",
        "variance",
        "var_pop",
        "var_samp",
    ];
    const MYSQL: &[&str] = &[
        "date_add",
        "date_format",
        "date_sub",
        "datediff",
        "day",
        "from_unixtime",
        "instr",
        "locate",
        "month",
        "sha1",
        "sha2",
        "timestampdiff",
        "unix_timestamp",
        "year",
    ];
    const POSTGRESQL: &[&str] = &[
        "json_extract_path",
        "json_extract_path_text",
        "jsonb_extract_path",
        "jsonb_extract_path_text",
        "pg_typeof",
        "to_char",
        "to_date",
        "to_timestamp",
    ];
    COMMON.contains(&name)
        || match engine {
            Engine::Mysql => MYSQL.contains(&name),
            Engine::Postgresql => POSTGRESQL.contains(&name),
        }
}

fn parse_and_tokenize<D: Dialect>(
    dialect: &D,
    statement: &str,
) -> Result<(Vec<sqlparser::ast::Statement>, Vec<Token>), SqlPolicyError> {
    let statements = Parser::parse_sql(dialect, statement)
        .map_err(|error| SqlPolicyError::Parse(error.to_string()))?;
    let tokens = Tokenizer::new(dialect, statement)
        .tokenize()
        .map_err(|error| SqlPolicyError::Parse(error.to_string()))?;
    Ok((statements, tokens))
}

fn reject_blocked_tokens(tokens: &[Token]) -> Result<(), SqlPolicyError> {
    const BLOCKED: &[&str] = &[
        "ALTER",
        "ANALYZE",
        "ATTACH",
        "CALL",
        "COMMIT",
        "COPY",
        "CREATE",
        "DELETE",
        "DETACH",
        "DROP",
        "EXECUTE",
        "GRANT",
        "INSERT",
        "INTO",
        "LOAD",
        "LOCK",
        "MERGE",
        "PRAGMA",
        "REPLACE",
        "REVOKE",
        "ROLLBACK",
        "SET",
        "TRANSACTION",
        "TRUNCATE",
        "UNLOCK",
        "UPDATE",
        "USE",
    ];
    if tokens
        .iter()
        .any(|token| matches!(token, Token::Assignment))
    {
        return Err(SqlPolicyError::Construct("assignment".to_owned()));
    }
    if tokens
        .iter()
        .any(|token| matches!(token, Token::AtSign | Token::AtAt))
    {
        return Err(SqlPolicyError::Construct(
            "session or system variable".to_owned(),
        ));
    }
    if tokens.iter().any(|token| {
        matches!(
            token,
            Token::Whitespace(
                Whitespace::SingleLineComment { .. } | Whitespace::MultiLineComment(_)
            )
        )
    }) {
        return Err(SqlPolicyError::Construct("comment".to_owned()));
    }

    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|token| !matches!(token, Token::Whitespace(_)))
        .collect();
    for (index, token) in significant.iter().enumerate() {
        if let Token::Word(word) = token {
            if word.value.starts_with('@') {
                return Err(SqlPolicyError::Construct(
                    "session or system variable".to_owned(),
                ));
            }
            let keyword = word.value.to_ascii_uppercase();
            if BLOCKED.contains(&keyword.as_str()) {
                return Err(SqlPolicyError::Construct(keyword.to_ascii_lowercase()));
            }
            if keyword == "FOR"
                && significant.get(index + 1).is_some_and(|next| {
                    matches!(next, Token::Word(next_word) if next_word.value.eq_ignore_ascii_case("SHARE"))
                })
            {
                return Err(SqlPolicyError::Construct("for share".to_owned()));
            }
        }
    }
    Ok(())
}
