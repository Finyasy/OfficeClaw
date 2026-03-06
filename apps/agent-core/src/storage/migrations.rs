use sqlx::Row;

use crate::storage::db::Database;

pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "0001_init",
        sql: include_str!("../../../../db/migrations/0001_init.sql"),
    },
    Migration {
        version: "0002_indexes",
        sql: include_str!("../../../../db/migrations/0002_indexes.sql"),
    },
];

pub async fn run(database: &Database) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        create table if not exists schema_migrations (
            version text primary key,
            applied_at timestamptz not null default now()
        )
        "#,
    )
    .execute(database.pool())
    .await?;

    for migration in MIGRATIONS {
        if already_applied(database, migration.version).await? {
            continue;
        }

        let mut tx = database.pool().begin().await?;
        sqlx::raw_sql(migration.sql).execute(&mut *tx).await?;
        sqlx::query(
            r#"
            insert into schema_migrations (version)
            values ($1)
            "#,
        )
        .bind(migration.version)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }

    Ok(())
}

async fn already_applied(database: &Database, version: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        r#"
        select version
        from schema_migrations
        where version = $1
        "#,
    )
    .bind(version)
    .fetch_optional(database.pool())
    .await?;

    Ok(row
        .map(|value| value.try_get::<String, _>("version"))
        .transpose()?
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::MIGRATIONS;

    #[test]
    fn migrations_are_declared_in_ascending_order() {
        let versions = MIGRATIONS
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>();
        assert_eq!(versions, vec!["0001_init", "0002_indexes"]);
    }
}
