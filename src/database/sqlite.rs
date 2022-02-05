use std::time::Duration;

use async_trait::async_trait;
use futures::TryStreamExt;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Column as _, Row as _};

use database_tree::{Child, Database, Table};

use crate::database::{convert_column_val_to_str, Column, Constraint, ForeignKey, Index};
use crate::pool_exec_impl;

use super::{ExecuteResult, Pool, TableRow, RECORDS_LIMIT_PER_PAGE};

pub struct SqlitePool {
    pool: sqlx::sqlite::SqlitePool,
}

impl SqlitePool {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            pool: SqlitePoolOptions::new()
                .connect_timeout(Duration::from_secs(5))
                .connect(database_url)
                .await?,
        })
    }
}

#[async_trait]
impl Pool for SqlitePool {
    async fn execute(&self, query: &String) -> anyhow::Result<ExecuteResult> {
        pool_exec_impl!(&self.pool, query);
    }

    async fn get_databases(&self) -> anyhow::Result<Vec<Database>> {
        let databases = sqlx::query("SELECT name FROM pragma_database_list")
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

    async fn get_tables(&self, _database: String) -> anyhow::Result<Vec<Child>> {
        let mut rows =
            sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table'").fetch(&self.pool);
        let mut tables = Vec::new();
        while let Some(row) = rows.try_next().await? {
            tables.push(Table {
                name: row.try_get("name")?,
                create_time: None,
                update_time: None,
                engine: None,
                schema: None,
            })
        }
        Ok(tables.into_iter().map(|table| table.into()).collect())
    }

    async fn get_records(
        &self,
        _database: &Database,
        table: &Table,
        page: u16,
        filter: Option<String>,
    ) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
        let query = if let Some(filter) = filter {
            format!(
                "SELECT * FROM `{table}` WHERE {filter} LIMIT {page}, {limit}",
                table = table.name,
                filter = filter,
                page = page,
                limit = RECORDS_LIMIT_PER_PAGE
            )
        } else {
            format!(
                "SELECT * FROM `{}` LIMIT {page}, {limit}",
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
        _database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let query = format!("SELECT * FROM pragma_table_info('{}');", table.name);
        let mut rows = sqlx::query(query.as_str()).fetch(&self.pool);
        let mut columns: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            let null: Option<i16> = row.try_get("notnull")?;
            columns.push(Box::new(Column {
                name: row.try_get("name")?,
                r#type: row.try_get("type")?,
                null: if matches!(null, Some(null) if null == 1) {
                    Some("✔︎".to_string())
                } else {
                    Some("".to_string())
                },
                default: row.try_get("dflt_value")?,
                comment: None,
            }))
        }
        Ok(columns)
    }

    async fn get_constraints(
        &self,
        _database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let mut rows = sqlx::query(
            "
            SELECT
                p.origin,
                s.name AS index_name,
                i.name AS column_name
            FROM
                sqlite_master s
                JOIN pragma_index_list(s.tbl_name) p ON s.name = p.name,
                pragma_index_info(s.name) i
            WHERE
                s.type = 'index'
                AND tbl_name = ?
                AND NOT p.origin = 'c'
            ",
        )
        .bind(&table.name)
        .fetch(&self.pool);
        let mut constraints: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            constraints.push(Box::new(Constraint {
                name: row.try_get("index_name")?,
                column_name: row.try_get("column_name")?,
                origin: row.try_get("origin")?,
            }))
        }
        Ok(constraints)
    }

    async fn get_foreign_keys(
        &self,
        _database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let query = format!(
            "SELECT p.`from`, p.`to`, p.`table` FROM pragma_foreign_key_list('{}') p",
            &table.name
        );
        let mut rows = sqlx::query(query.as_str())
            .bind(&table.name)
            .fetch(&self.pool);
        let mut foreign_keys: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            foreign_keys.push(Box::new(ForeignKey {
                column_name: row.try_get("from")?,
                ref_table: row.try_get("table")?,
                ref_column: row.try_get("to")?,
                name: None,
            }))
        }
        Ok(foreign_keys)
    }

    async fn get_indexes(
        &self,
        _database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>> {
        let mut rows = sqlx::query(
            "
            SELECT
                m.name AS index_name,
                p.*
            FROM
                sqlite_master m,
                pragma_index_info(m.name) p
            WHERE
                m.type = 'index'
                AND m.tbl_name = ?
            ",
        )
        .bind(&table.name)
        .fetch(&self.pool);
        let mut foreign_keys: Vec<Box<dyn TableRow>> = vec![];
        while let Some(row) = rows.try_next().await? {
            foreign_keys.push(Box::new(Index {
                name: row.try_get("index_name")?,
                column_name: row.try_get("name")?,
                r#type: Some(String::new()),
            }))
        }
        Ok(foreign_keys)
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
