use async_trait::async_trait;
use sqlx::mysql::MySqlRow;
use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use sqlx::TypeInfo as _;

use database_tree::{Child, Database, Table};
pub use mysql::MySqlPool;
pub use postgres::PostgresPool;
pub use sqlite::SqlitePool;

pub mod mysql;
pub mod postgres;
pub mod sqlite;

pub const RECORDS_LIMIT_PER_PAGE: u8 = 200;

#[async_trait]
pub trait Pool: Send + Sync {
    async fn execute(&self, query: &String) -> anyhow::Result<ExecuteResult>;
    async fn get_databases(&self) -> anyhow::Result<Vec<Database>>;
    // TODO: Change argument to &String
    async fn get_tables(&self, database: String) -> anyhow::Result<Vec<Child>>;
    async fn get_records(
        &self,
        database: &Database,
        table: &Table,
        page: u16,
        filter: Option<String>,
    ) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)>;
    async fn get_columns(&self, table: &Table) -> anyhow::Result<Vec<Column>>;
    async fn get_constraints(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>>;
    async fn get_foreign_keys(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>>;
    async fn get_indexes(
        &self,
        database: &Database,
        table: &Table,
    ) -> anyhow::Result<Vec<Box<dyn TableRow>>>;
    async fn close(&self);

    async fn get_keywords(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![
            "IN", "AND", "OR", "NOT", "NULL", "IS", "SELECT", "INSERT", "UPDATE", "DELETE", "FROM",
            "LIMIT", "WHERE", "LIKE",
        ]
        .into_iter()
        .map(|s| String::from(s))
        .collect())
    }
}

pub enum ExecuteResult {
    Read {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        database: Database,
        table: Table,
    },
    Write {
        updated_rows: u64,
    },
}
pub trait TableRow: std::marker::Send {
    fn fields(&self) -> Vec<String>;
    fn columns(&self) -> Vec<String>;
}

pub struct Column {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub null: Option<String>,
    pub default: Option<String>,
    pub comment: Option<String>,
}

impl TableRow for Column {
    fn fields(&self) -> Vec<String> {
        vec![
            "name".to_string(),
            "type".to_string(),
            "null".to_string(),
            "default".to_string(),
            "comment".to_string(),
        ]
    }

    fn columns(&self) -> Vec<String> {
        vec![
            self.name
                .as_ref()
                .map_or(String::new(), |name| name.to_string()),
            self.r#type
                .as_ref()
                .map_or(String::new(), |r#type| r#type.to_string()),
            self.null
                .as_ref()
                .map_or(String::new(), |null| null.to_string()),
            self.default
                .as_ref()
                .map_or(String::new(), |default| default.to_string()),
            self.comment
                .as_ref()
                .map_or(String::new(), |comment| comment.to_string()),
        ]
    }
}

pub struct Index {
    name: Option<String>,
    column_name: Option<String>,
    r#type: Option<String>,
}

impl TableRow for Index {
    fn fields(&self) -> Vec<String> {
        vec![
            "name".to_string(),
            "column_name".to_string(),
            "type".to_string(),
        ]
    }

    fn columns(&self) -> Vec<String> {
        vec![
            self.name
                .as_ref()
                .map_or(String::new(), |name| name.to_string()),
            self.column_name
                .as_ref()
                .map_or(String::new(), |column_name| column_name.to_string()),
            self.r#type
                .as_ref()
                .map_or(String::new(), |r#type| r#type.to_string()),
        ]
    }
}

pub struct ForeignKey {
    name: Option<String>,
    column_name: Option<String>,
    ref_table: Option<String>,
    ref_column: Option<String>,
}

impl TableRow for ForeignKey {
    fn fields(&self) -> Vec<String> {
        vec![
            "name".to_string(),
            "column_name".to_string(),
            "ref_table".to_string(),
            "ref_column".to_string(),
        ]
    }

    fn columns(&self) -> Vec<String> {
        vec![
            self.name
                .as_ref()
                .map_or(String::new(), |name| name.to_string()),
            self.column_name
                .as_ref()
                .map_or(String::new(), |r#type| r#type.to_string()),
            self.ref_table
                .as_ref()
                .map_or(String::new(), |r#type| r#type.to_string()),
            self.ref_column
                .as_ref()
                .map_or(String::new(), |r#type| r#type.to_string()),
        ]
    }
}

pub struct Constraint {
    name: String,
    column_name: String,
    origin: Option<String>,
}

impl TableRow for Constraint {
    fn fields(&self) -> Vec<String> {
        let mut fields = vec!["name".to_string(), "column_name".to_string()];
        if self.origin.is_some() {
            fields.push("origin".to_string());
        }
        fields
    }

    fn columns(&self) -> Vec<String> {
        let mut columns = vec![self.name.to_string(), self.column_name.to_string()];
        if let Some(origin) = &self.origin {
            columns.push(origin.clone())
        }
        columns
    }
}

#[macro_export]
macro_rules! pool_exec_impl {
    ($pool : expr, $query : expr) => {
        use log::debug;
        let query = $query.trim();
        debug!("Executing query {}", query);
        let mut result_sets = sqlx::query(query).fetch_many($pool);
        let mut headers = vec![];
        let mut records = vec![];

        while let Some(r) = result_sets.try_next().await? {
            debug!(
                "Query result is {}",
                if r.is_left() {
                    "write operation"
                } else {
                    "row"
                }
            );
            if r.is_left() && records.is_empty() {
                debug!("Returning ExecuteResult::Write");
                return Ok(ExecuteResult::Write {
                    updated_rows: r.left().unwrap().rows_affected(),
                });
            } else if let Some(row) = r.right() {
                if headers.is_empty() {
                    headers = row
                        .columns()
                        .iter()
                        .map(|column| column.name().to_string())
                        .collect();
                }
                let mut new_row = vec![];
                for column in row.columns() {
                    new_row.push(convert_column_val_to_str(&row, column)?)
                }
                records.push(new_row)
            }
        }
        debug!("Returning ExecuteResult::Read");
        return Ok(ExecuteResult::Read {
            headers,
            rows: records,
            database: Database {
                name: "-".to_string(),
                children: Vec::new(),
            },
            table: Table {
                name: "-".to_string(),
                create_time: None,
                update_time: None,
                engine: None,
                schema: None,
                database: None,
            },
        });
    };
}

// #[macro_export]
// macro_rules! get_or_null {
//     ($value:expr) => {
//         $value.map_or("NULL".to_string(), |v| v.to_string())
//     };
// }
#[inline(always)]
fn get_or_null<T: ToString>(val: Option<T>) -> String {
    val.map_or("NULL".to_string(), |v| v.to_string())
}

macro_rules! convert_column {
    ($row : expr, $column_name : expr, $($typ : ty),+) => {
        $(
        if let Ok(value) = $row.try_get($column_name) {
            let value : Option<$typ> = value;
            return Ok(get_or_null(value))
        }
        )+
    };
}

macro_rules! convert_column_to_common_types {
    ($row : expr, $column_name : expr) => {
        convert_column!(
            $row,
            $column_name,
            String,
            &str,
            i8,
            i16,
            i32,
            i64,
            u32,
            f32,
            f64,
            chrono::DateTime<chrono::Utc>,
            chrono::NaiveDateTime,
            chrono::DateTime<chrono::Local>,
            chrono::NaiveDate,
            chrono::NaiveTime,
            serde_json::Value,
            bool
        );
    };
}

pub fn convert_column_val_to_str<R: sqlx::Row + std::any::Any, C: sqlx::Column>(
    row: &R,
    column: &C,
) -> anyhow::Result<String> {
    let row: &dyn std::any::Any = row;
    let column_name = column.name();
    if let Some(row) = row.downcast_ref::<MySqlRow>() {
        convert_column_to_common_types!(row, column_name);
        convert_column!(row, column_name, rust_decimal::Decimal, u16, u64);
        // convert_column(row, column_name, u64);
    } else if let Some(row) = row.downcast_ref::<SqliteRow>() {
        convert_column_to_common_types!(row, column_name);
        convert_column!(row, column_name, u16);
    } else if let Some(row) = row.downcast_ref::<PgRow>() {
        convert_column_to_common_types!(row, column_name);
        convert_column!(row, column_name, rust_decimal::Decimal);
        if let Ok(value) = row.try_get(column_name) {
            let value: Option<&[u8]> = value;
            return Ok(value.map_or("NULL".to_string(), |values| {
                format!(
                    "\\x{}",
                    values
                        .iter()
                        .map(|v| format!("{:02x}", v))
                        .collect::<String>()
                )
            }));
        }
        if let Ok(value) = row.try_get(column_name) {
            let value: Option<Vec<String>> = value;
            return Ok(value.map_or("NULL".to_string(), |v| v.join(",")));
        }
    }
    anyhow::bail!(
        "column type not implemented: `{}` {}",
        column_name,
        column.type_info().clone().name()
    )
}
