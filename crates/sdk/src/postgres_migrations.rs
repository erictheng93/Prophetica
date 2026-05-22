use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

use sqlx::{
    Connection, Executor, PgConnection, PgPool,
    migrate::{AppliedMigration, Migrate, MigrateError, Migration, Migrator},
    query, query_as, query_scalar,
};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub const USER_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_user_service";
pub const API_GATEWAY_MIGRATION_TABLE: &str = "_sqlx_migrations_api_gateway";
pub const REFERRAL_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_referral_service";
pub const KYC_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_kyc_service";
pub const POSITION_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_position_service";
pub const WALLET_LEDGER_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_wallet_ledger_service";

/// Shared migration table used before per-service isolation. A database created by an older
/// build records every service's migrations here; [`adopt_legacy_migrations`] bridges those rows
/// into the service-specific table so already-applied migrations are not re-run.
const LEGACY_MIGRATION_TABLE: &str = "_sqlx_migrations";

struct OwnedMigration {
    version: i64,
    description: String,
    checksum: Vec<u8>,
}

/// Runs `SQLx` Postgres migrations while recording applied versions in a service-specific table.
///
/// Each service in a process that shares one database keeps its own migration-metadata table,
/// because the services' migration version numbers collide and a shared `_sqlx_migrations` table
/// cannot tell them apart.
///
/// ## Upgrading a database created before per-service isolation
///
/// Such a database has only the shared `_sqlx_migrations` table, so the service-specific table
/// starts empty — which would otherwise re-run every migration and fail on objects that already
/// exist. [`adopt_legacy_migrations`] bridges this: for every migration the legacy table already
/// applied successfully under the same `(version, description)`, it records an applied row in the
/// service-specific table using the *current* migration's checksum. The legacy table is never
/// modified. This bridge is a one-way legacy upgrade path, not a license to edit applied
/// migrations — merged migrations stay immutable and schema changes ship as new forward
/// migrations.
///
/// # Errors
///
/// Returns an error when the migration table name is not a safe SQL identifier, when acquiring a
/// database connection fails, or when `SQLx` fails to run or validate a migration.
pub async fn run_postgres_migrations(
    migrator: Migrator,
    pool: &PgPool,
    table_name: &str,
) -> anyhow::Result<()> {
    let table_name = migration_table_identifier(table_name)?;
    let owned_migrations = migrator
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .map(|migration| OwnedMigration {
            version: migration.version,
            description: migration.description.to_string(),
            checksum: migration.checksum.to_vec(),
        })
        .collect();
    let mut conn = pool.acquire().await?;
    let mut conn = MigrationTableConnection {
        conn: &mut conn,
        table_name,
        owned_migrations,
    };

    // SQLx 0.8 has no stable migration-table setter. This wrapper implements
    // `Migrate` with service-specific bookkeeping and uses `run_direct` as the
    // narrow escape hatch that bypasses the default `_sqlx_migrations` table.
    migrator.run_direct(&mut conn).await?;

    Ok(())
}

fn migration_table_identifier(table_name: &str) -> anyhow::Result<String> {
    let mut chars = table_name.chars();
    let Some(first) = chars.next() else {
        anyhow::bail!("migration table name must not be empty");
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        anyhow::bail!("migration table name must start with '_' or an ASCII letter");
    }
    if chars.any(|char| !(char == '_' || char.is_ascii_alphanumeric())) {
        anyhow::bail!(
            "migration table name must contain only ASCII letters, digits, and underscores"
        );
    }

    Ok(format!("\"{table_name}\""))
}

struct MigrationTableConnection<'c> {
    conn: &'c mut PgConnection,
    table_name: String,
    /// This service's up-migrations (version, description, current checksum), used to adopt
    /// already-applied rows from the legacy shared migration table.
    owned_migrations: Vec<OwnedMigration>,
}

impl Migrate for MigrationTableConnection<'_> {
    fn ensure_migrations_table(&mut self) -> BoxFuture<'_, Result<(), MigrateError>> {
        Box::pin(async move {
            self.conn
                .execute(&*format!(
                    "
CREATE TABLE IF NOT EXISTS {table_name} (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL,
    checksum BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
);
                        ",
                    table_name = self.table_name
                ))
                .await?;

            adopt_legacy_migrations(&mut *self.conn, &self.table_name, &self.owned_migrations)
                .await?;

            Ok(())
        })
    }

    fn dirty_version(&mut self) -> BoxFuture<'_, Result<Option<i64>, MigrateError>> {
        Box::pin(async move {
            let row: Option<(i64,)> = query_as(&format!(
                "SELECT version FROM {} WHERE success = false ORDER BY version LIMIT 1",
                self.table_name
            ))
            .fetch_optional(&mut *self.conn)
            .await?;

            Ok(row.map(|row| row.0))
        })
    }

    fn list_applied_migrations(
        &mut self,
    ) -> BoxFuture<'_, Result<Vec<AppliedMigration>, MigrateError>> {
        Box::pin(async move {
            let rows: Vec<(i64, Vec<u8>)> = query_as(&format!(
                "SELECT version, checksum FROM {} ORDER BY version",
                self.table_name
            ))
            .fetch_all(&mut *self.conn)
            .await?;

            Ok(rows
                .into_iter()
                .map(|(version, checksum)| AppliedMigration {
                    version,
                    checksum: checksum.into(),
                })
                .collect())
        })
    }

    fn lock(&mut self) -> BoxFuture<'_, Result<(), MigrateError>> {
        Box::pin(async move { self.conn.lock().await })
    }

    fn unlock(&mut self) -> BoxFuture<'_, Result<(), MigrateError>> {
        Box::pin(async move { self.conn.unlock().await })
    }

    fn apply<'e: 'm, 'm>(
        &'e mut self,
        migration: &'m Migration,
    ) -> BoxFuture<'m, Result<Duration, MigrateError>> {
        Box::pin(async move {
            let start = Instant::now();

            if migration.no_tx {
                execute_migration(self.conn, migration, &self.table_name).await?;
            } else {
                let mut tx = self.conn.begin().await?;
                execute_migration(&mut tx, migration, &self.table_name).await?;
                tx.commit().await?;
            }

            let elapsed = start.elapsed();

            #[allow(clippy::cast_possible_truncation)]
            query(&format!(
                "UPDATE {} SET execution_time = $1 WHERE version = $2",
                self.table_name
            ))
            .bind(elapsed.as_nanos() as i64)
            .bind(migration.version)
            .execute(&mut *self.conn)
            .await?;

            Ok(elapsed)
        })
    }

    fn revert<'e: 'm, 'm>(
        &'e mut self,
        migration: &'m Migration,
    ) -> BoxFuture<'m, Result<Duration, MigrateError>> {
        Box::pin(async move {
            let start = Instant::now();

            if migration.no_tx {
                revert_migration(self.conn, migration, &self.table_name).await?;
            } else {
                let mut tx = self.conn.begin().await?;
                revert_migration(&mut tx, migration, &self.table_name).await?;
                tx.commit().await?;
            }

            Ok(start.elapsed())
        })
    }
}

/// Bridges a database created before per-service migration isolation into the service-specific
/// migration table.
///
/// A pre-isolation database records every service's migrations in the single shared
/// `_sqlx_migrations` table. On the first run with isolated tables the service-specific table is
/// empty, so without this bridge every migration would be re-applied and fail on objects that
/// already exist.
///
/// For each of this service's migrations, if the legacy table holds a successful row with the
/// same `(version, description)`, that migration is recorded as applied in the service-specific
/// table using the *current* migration's checksum. Matching on `description` (derived from the
/// migration filename) rather than `checksum`:
///   * keeps the bridge working when a migration's body was edited after being applied, and
///   * still distinguishes services whose migrations collide on version numbers — a version slot
///     the legacy table recorded for a different service has a different description and is left
///     for that service to apply normally.
///
/// Rows are inserted only for versions the service-specific table does not already record
/// (`ON CONFLICT DO NOTHING`), and the legacy table itself is never modified or dropped.
///
/// This is strictly a one-way legacy upgrade bridge. It is NOT a sanctioned way to mutate an
/// applied migration: a merged migration must stay immutable, and schema changes must ship as
/// new forward migrations.
async fn adopt_legacy_migrations(
    conn: &mut PgConnection,
    table_name: &str,
    owned_migrations: &[OwnedMigration],
) -> Result<(), MigrateError> {
    let legacy_present: bool = query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(LEGACY_MIGRATION_TABLE)
        .fetch_one(&mut *conn)
        .await?;

    if !legacy_present {
        return Ok(());
    }

    let adopt_sql = format!(
        "
INSERT INTO {table_name} ( version, description, installed_on, success, checksum, execution_time )
SELECT $1, $2, legacy.installed_on, TRUE, $3, legacy.execution_time
FROM {LEGACY_MIGRATION_TABLE} AS legacy
WHERE legacy.version = $1 AND legacy.description = $2 AND legacy.success = TRUE
ON CONFLICT (version) DO NOTHING
        "
    );

    for migration in owned_migrations {
        let result = query(&adopt_sql)
            .bind(migration.version)
            .bind(&migration.description)
            .bind(migration.checksum.as_slice())
            .execute(&mut *conn)
            .await?;

        if result.rows_affected() > 0 {
            tracing::info!(
                version = migration.version,
                description = %migration.description,
                table = table_name,
                "adopted already-applied migration from the legacy shared _sqlx_migrations table"
            );
        }
    }

    Ok(())
}

async fn execute_migration(
    conn: &mut PgConnection,
    migration: &Migration,
    table_name: &str,
) -> Result<(), MigrateError> {
    conn.execute(&*migration.sql)
        .await
        .map_err(|error| MigrateError::ExecuteMigration(error, migration.version))?;

    query(&format!(
        "
INSERT INTO {table_name} ( version, description, success, checksum, execution_time )
VALUES ( $1, $2, TRUE, $3, -1 )
        "
    ))
    .bind(migration.version)
    .bind(&*migration.description)
    .bind(&*migration.checksum)
    .execute(conn)
    .await?;

    Ok(())
}

async fn revert_migration(
    conn: &mut PgConnection,
    migration: &Migration,
    table_name: &str,
) -> Result<(), MigrateError> {
    conn.execute(&*migration.sql)
        .await
        .map_err(|error| MigrateError::ExecuteMigration(error, migration.version))?;

    query(&format!("DELETE FROM {table_name} WHERE version = $1"))
        .bind(migration.version)
        .execute(conn)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        API_GATEWAY_MIGRATION_TABLE, KYC_SERVICE_MIGRATION_TABLE, LEGACY_MIGRATION_TABLE,
        POSITION_SERVICE_MIGRATION_TABLE, REFERRAL_SERVICE_MIGRATION_TABLE,
        USER_SERVICE_MIGRATION_TABLE, WALLET_LEDGER_SERVICE_MIGRATION_TABLE,
        migration_table_identifier,
    };

    #[test]
    fn service_migration_tables_are_distinct() {
        let tables = [
            USER_SERVICE_MIGRATION_TABLE,
            API_GATEWAY_MIGRATION_TABLE,
            REFERRAL_SERVICE_MIGRATION_TABLE,
            KYC_SERVICE_MIGRATION_TABLE,
            POSITION_SERVICE_MIGRATION_TABLE,
            WALLET_LEDGER_SERVICE_MIGRATION_TABLE,
        ];

        assert_eq!(
            tables.into_iter().collect::<HashSet<_>>().len(),
            tables.len()
        );
    }

    #[test]
    fn service_migration_tables_differ_from_legacy_table() {
        for table in [
            USER_SERVICE_MIGRATION_TABLE,
            API_GATEWAY_MIGRATION_TABLE,
            REFERRAL_SERVICE_MIGRATION_TABLE,
            KYC_SERVICE_MIGRATION_TABLE,
            POSITION_SERVICE_MIGRATION_TABLE,
            WALLET_LEDGER_SERVICE_MIGRATION_TABLE,
        ] {
            assert_ne!(table, LEGACY_MIGRATION_TABLE);
        }
    }

    #[test]
    fn migration_table_identifier_rejects_unsafe_names() {
        assert!(migration_table_identifier("_sqlx_migrations_user_service").is_ok());
        assert!(migration_table_identifier("sqlx_migrations_1").is_ok());

        assert!(migration_table_identifier("").is_err());
        assert!(migration_table_identifier("1_sqlx_migrations").is_err());
        assert!(migration_table_identifier("_sqlx_migrations; DROP TABLE users").is_err());
        assert!(migration_table_identifier("_sqlx_migrations.user_service").is_err());
    }
}
