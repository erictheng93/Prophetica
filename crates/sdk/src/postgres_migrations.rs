use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

use sqlx::{
    Connection, Executor, PgConnection, PgPool,
    migrate::{AppliedMigration, Migrate, MigrateError, Migration, Migrator},
    query, query_as,
};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub const USER_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_user_service";
pub const API_GATEWAY_MIGRATION_TABLE: &str = "_sqlx_migrations_api_gateway";
pub const REFERRAL_SERVICE_MIGRATION_TABLE: &str = "_sqlx_migrations_referral_service";

/// Runs `SQLx` Postgres migrations while recording applied versions in a service-specific table.
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
    let mut conn = pool.acquire().await?;
    let mut conn = MigrationTableConnection {
        conn: &mut conn,
        table_name,
    };

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
        API_GATEWAY_MIGRATION_TABLE, REFERRAL_SERVICE_MIGRATION_TABLE,
        USER_SERVICE_MIGRATION_TABLE, migration_table_identifier,
    };

    #[test]
    fn service_migration_tables_are_distinct() {
        let tables = [
            USER_SERVICE_MIGRATION_TABLE,
            API_GATEWAY_MIGRATION_TABLE,
            REFERRAL_SERVICE_MIGRATION_TABLE,
        ];

        assert_eq!(
            tables.into_iter().collect::<HashSet<_>>().len(),
            tables.len()
        );
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
