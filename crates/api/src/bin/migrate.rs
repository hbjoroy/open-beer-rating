//! Explicit database maintenance command; the API never invokes it at startup.
use sqlx::{PgPool, postgres::PgPoolOptions};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        // Do not print connection errors: malformed connection strings may embed
        // credentials in their diagnostics. Never log DATABASE_URL itself.
        eprintln!("{message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    if let Some(option) = args.next() {
        if option != "--env-file" {
            return Err("Usage: migrate [--env-file PATH]".into());
        }
        let path = args.next().ok_or("Usage: migrate [--env-file PATH]")?;
        if args.next().is_some() {
            return Err("Usage: migrate [--env-file PATH]".into());
        }
        // Only load an explicitly chosen file, and keep existing environment
        // values. An ambient .env must not silently choose a maintenance target.
        dotenvy::from_path(path)
            .map_err(|_| "Could not load the selected dotenv file.".to_owned())?;
    }

    let database_url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or("DATABASE_URL must be set; optionally select a local file with --env-file PATH.")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .map_err(|_| "Could not connect to the database selected by DATABASE_URL.".to_owned())?;

    reject_unverified_script_history(&pool).await?;
    MIGRATOR.run(&pool).await.map_err(|_| {
        "SQLx migration failed. Check database availability, migration checksums and schema state; restore or reconcile from a verified backup before retrying. No fallback was attempted.".to_owned()
    })?;
    pool.close().await;
    println!("SQLx migrations completed.");
    Ok(())
}

async fn reject_unverified_script_history(pool: &PgPool) -> Result<(), String> {
    let (has_script_tracker, has_sqlx_tracker): (bool, bool) = sqlx::query_as(
        "SELECT to_regclass('_script_migrations') IS NOT NULL, to_regclass('_sqlx_migrations') IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| "Could not inspect migration history; no migrations were run.".to_owned())?;
    if !has_script_tracker {
        return Ok(());
    }

    let unverified_version: Option<(i64,)> = if has_sqlx_tracker {
        sqlx::query_as(
            "SELECT legacy.version FROM _script_migrations legacy \
             LEFT JOIN _sqlx_migrations verified \
             ON verified.version = legacy.version AND verified.success \
             WHERE verified.version IS NULL ORDER BY legacy.version LIMIT 1",
        )
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query_as("SELECT version FROM _script_migrations ORDER BY version LIMIT 1")
            .fetch_optional(pool)
            .await
    }
    .map_err(|_| "Could not verify legacy migration history; no migrations were run.".to_owned())?;

    if let Some((version,)) = unverified_version {
        return Err(format!(
            "Refusing migration: _script_migrations records version {version} without a corresponding successful SQLx migration. The old script stored no checksums and may have applied SQL partially. Back up this database, stop writers, and have an operator verify its schema and data against the historical migrations and a trusted backup. Restore a verified SQLx-managed backup or rebuild a fresh SQLx-managed database and transfer verified data. Do not delete trackers, replay migrations with Force, or fabricate SQLx checksums to bypass this guard."
        ));
    }
    // SQLx validates the corresponding checksums before applying pending SQL.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = false)]
    #[ignore = "requires dedicated PostgreSQL test server"]
    async fn legacy_history_requires_successful_sqlx_entries(pool: PgPool) {
        // A fresh database and an empty legacy tracker need no adoption.
        reject_unverified_script_history(&pool).await.unwrap();
        sqlx::query("CREATE TABLE _script_migrations (version BIGINT PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        reject_unverified_script_history(&pool).await.unwrap();
        sqlx::query("INSERT INTO _script_migrations VALUES (20260508000001)")
            .execute(&pool)
            .await
            .unwrap();
        let error = reject_unverified_script_history(&pool).await.unwrap_err();
        assert!(error.contains("20260508000001"));
        assert!(error.contains("without a corresponding successful SQLx migration"));

        sqlx::query(
            "CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, success BOOLEAN NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(reject_unverified_script_history(&pool).await.is_err());
        sqlx::query("INSERT INTO _sqlx_migrations VALUES (20260508000001, false)")
            .execute(&pool)
            .await
            .unwrap();
        assert!(reject_unverified_script_history(&pool).await.is_err());
        sqlx::query("UPDATE _sqlx_migrations SET success = true")
            .execute(&pool)
            .await
            .unwrap();
        reject_unverified_script_history(&pool).await.unwrap();

        // A verified prefix must not conceal later unverified legacy versions.
        sqlx::query("INSERT INTO _script_migrations VALUES (20260509000001)")
            .execute(&pool)
            .await
            .unwrap();
        let error = reject_unverified_script_history(&pool).await.unwrap_err();
        assert!(error.contains("20260509000001"));
    }
}
