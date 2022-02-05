use std::time::Duration;

use async_trait::async_trait;

use futures::TryStreamExt;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{Column as _, Row as _};

use database_tree::{Child, Database, Table};

use crate::database::{convert_column_val_to_str, Column, Constraint, ForeignKey, Index};
use crate::pool_exec_impl;

use super::{ExecuteResult, Pool, TableRow, RECORDS_LIMIT_PER_PAGE};

pub struct MySqlPool {
    pool: sqlx::mysql::MySqlPool,
}

impl MySqlPool {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            pool: MySqlPoolOptions::new()
                .connect_timeout(Duration::from_secs(5))
                .connect(database_url)
                .await?,
        })
    }
}

#[async_trait]
impl Pool for MySqlPool {
    async fn execute(&self, query: &String) -> anyhow::Result<ExecuteResult> {
        pool_exec_impl!(&self.pool, query);
    }

    async fn get_databases(&self) -> anyhow::Result<Vec<Database>> {
        let databases = sqlx::query("SHOW DATABASES")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|table| table.get(0))
            .collect::<Vec<String>>();
        let mut list = vec![];
        for db in databases {
            list.push(Database::new(
                db.clone(),
                self.get_tables(db.clone()).await?,
            ))
        }
        Ok(list)
    }

    async fn get_tables(&self, database: String) -> anyhow::Result<Vec<Child>> {
        let query = format!("SHOW TABLE STATUS FROM `{}`", database);
        let mut rows = sqlx::query(query.as_str()).fetch(&self.pool);
        let mut tables = vec![];
        while let Some(row) = rows.try_next().await? {
            tables.push(Table {
                name: row.try_get("Name")?,
                create_time: row.try_get("Create_time")?,
                update_time: row.try_get("Update_time")?,
                engine: row.try_get("Engine")?,
                schema: None,
            })
        }
        Ok(tables.into_iter().map(|table| table.into()).collect())
    }

    async fn get_records(
        &self,
        database: &Database,
        table: &Table,
        page: u16,
        filter: Option<String>,
    ) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
        let query = if let Some(filter) = filter {
            format!(
                "SELECT * FROM `{database}`.`{table}` WHERE {filter} LIMIT {page}, {limit}",
                database = database.name,
                table = table.name,
                filter = filter,
                page = page,
                limit = RECORDS_LIMIT_PER_PAGE
            )
        } else {
            format!(
                "SELECT * FROM `{}`.`{}` LIMIT {page}, {limit}",
                database.name,
                table.name,
                page = page,
                limit = RECORDS_LIMIT_PER_PAGE
            )
        };
        let mut rows = sqlx::query(query.as_str()).fetch(&self.pool);
        let mut headers = vec![];
        let mut records = vec![];
        while let Some(row) = rows.try_next().await? {
            headers = row
                .columns()
                .iter()
                .map(|column| column.name().to_string())
                .collect();
            let mut new_row = vec![];
            for column in row.columns() {
                new_row.push(convert_column_val_to_str(&row, column)?)
            }
            records.push(new_row)
        }
        Ok((headers, records))
    }

    async fn get_columns(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let query = format!(
            "SHOW FULL COLUMNS FROM `{}`.`{}`",
            database.name, table.name
        );
        let mut rows = sqlx::query(query.as_str()).fetch(&self.pool);
        let mut columns: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            columns.push(Box::new(Column {
                name: row.try_get("Field")?,
                r#type: row.try_get("Type")?,
                null: row.try_get("Null")?,
                default: row.try_get("Default")?,
                comment: row.try_get("Comment")?,
            }))
        }
        Ok(columns)
    }

    async fn get_constraints(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let mut rows = sqlx::query(
            "
        SELECT
            COLUMN_NAME,
            CONSTRAINT_NAME
        FROM
            information_schema.KEY_COLUMN_USAGE
        WHERE
            REFERENCED_TABLE_SCHEMA IS NULL
            AND REFERENCED_TABLE_NAME IS NULL
            AND TABLE_SCHEMA = ?
            AND TABLE_NAME = ?
        ",
        )
        .bind(&database.name)
        .bind(&table.name)
        .fetch(&self.pool);
        let mut constraints: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            constraints.push(Box::new(Constraint {
                name: row.try_get("CONSTRAINT_NAME")?,
                column_name: row.try_get("COLUMN_NAME")?,
                origin: None,
            }))
        }
        Ok(constraints)
    }

    async fn get_foreign_keys(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let mut rows = sqlx::query(
            "
        SELECT
            TABLE_NAME,
            COLUMN_NAME,
            CONSTRAINT_NAME,
            REFERENCED_TABLE_SCHEMA,
            REFERENCED_TABLE_NAME,
            REFERENCED_COLUMN_NAME
        FROM
            INFORMATION_SCHEMA.KEY_COLUMN_USAGE
        WHERE
            REFERENCED_TABLE_SCHEMA IS NOT NULL
            AND REFERENCED_TABLE_NAME IS NOT NULL
            AND TABLE_SCHEMA = ?
            AND TABLE_NAME = ?
        ",
        )
        .bind(&database.name)
        .bind(&table.name)
        .fetch(&self.pool);
        let mut foreign_keys: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            foreign_keys.push(Box::new(ForeignKey {
                name: row.try_get("CONSTRAINT_NAME")?,
                column_name: row.try_get("COLUMN_NAME")?,
                ref_table: row.try_get("REFERENCED_TABLE_NAME")?,
                ref_column: row.try_get("REFERENCED_COLUMN_NAME")?,
            }))
        }
        Ok(foreign_keys)
    }

    async fn get_indexes(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let mut rows = sqlx::query(
            "
        SELECT
            DISTINCT TABLE_NAME,
            INDEX_NAME,
            INDEX_TYPE,
            COLUMN_NAME
        FROM
            INFORMATION_SCHEMA.STATISTICS
        WHERE
            TABLE_SCHEMA = ?
            AND TABLE_NAME = ?
        ",
        )
        .bind(&database.name)
        .bind(&table.name)
        .fetch(&self.pool);
        let mut foreign_keys: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            foreign_keys.push(Box::new(Index {
                name: row.try_get("INDEX_NAME")?,
                column_name: row.try_get("COLUMN_NAME")?,
                r#type: row.try_get("INDEX_TYPE")?,
            }))
        }
        Ok(foreign_keys)
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
