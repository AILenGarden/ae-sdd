package ai.nubase.postgrest.multidb;

import lombok.extern.slf4j.Slf4j;

import javax.sql.DataSource;
import java.io.PrintWriter;
import java.sql.Connection;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.util.logging.Logger;

/**
 * Guardian DataSource - defensive DataSource
 * <p>
 * This DataSource is used as the defaultTargetDataSource for RoutingDataSource.
 * Any database operation that does not go through the normal tenant authentication
 * flow will immediately throw an exception, preventing accidental access to the
 * metadata database.
 * <p>
 * Use cases:
 * - During application startup, Hibernate initialization does not need a real database connection (only DDL checks)
 * - If a request bypasses UnifiedMultiTenancyFilter, it fails immediately instead of exposing the metadata database
 * - Prevents internal code from accidentally accessing the wrong DataSource
 *
 * @author security-team
 */
@Slf4j
public class GuardianDataSource implements DataSource {

    private static final String ERROR_MESSAGE =
            "No tenant context found! This is a security guard. " +
            "Database operations require proper tenant authentication via UnifiedMultiTenancyFilter. " +
            "Possible causes:\n" +
            "1. Request missing 'apikey' header\n" +
            "2. Request bypassed UnifiedMultiTenancyFilter\n" +
            "3. MultiTenancyContext not properly set\n" +
            "4. Internal code accessing RoutingDataSource without tenant context";

    public GuardianDataSource() {
        log.warn("⚠️ GuardianDataSource initialized - " +
                "any database operation without proper tenant context will be blocked");
    }

    @Override
    public Connection getConnection() throws SQLException {
        log.error("❌ SECURITY ALERT: Attempted to get connection from GuardianDataSource without tenant context");
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public Connection getConnection(String username, String password) throws SQLException {
        log.error("❌ SECURITY ALERT: Attempted to get connection from GuardianDataSource with credentials");
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public PrintWriter getLogWriter() throws SQLException {
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public void setLogWriter(PrintWriter out) throws SQLException {
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public void setLoginTimeout(int seconds) throws SQLException {
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public int getLoginTimeout() throws SQLException {
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public Logger getParentLogger() throws SQLFeatureNotSupportedException {
        throw new SQLFeatureNotSupportedException(ERROR_MESSAGE);
    }

    @Override
    public <T> T unwrap(Class<T> iface) throws SQLException {
        throw new SQLException(ERROR_MESSAGE);
    }

    @Override
    public boolean isWrapperFor(Class<?> iface) throws SQLException {
        throw new SQLException(ERROR_MESSAGE);
    }
}
