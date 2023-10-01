//! Rizz is a query builder and migration generator for sqlite. Don't call it an ORM.
//!
extern crate self as rizz;

pub use rizz_macros::{Row, Table};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use rizz::{and, connect, db, eq, or, Database, Error, Row, Table, Value};
    use serde::Deserialize;

    use crate::{count, star, Integer, Real, Text};

    type TestResult<T> = Result<T, Error>;

    async fn test_db() -> TestResult<Database> {
        let conn = connect(":memory:").await?;
        let db = db(conn);

        Ok(db)
    }

    impl std::fmt::Display for Value {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Value::Lit(str) => f.write_str(str),
                _ => f.write_str("?"),
            }
        }
    }

    trait ToSql {
        fn to_sql(&self) -> Value;
    }

    impl ToSql for Accounts {
        fn to_sql(&self) -> Value {
            Value::Lit(self.table_name())
        }
    }

    impl ToSql for &'static str {
        fn to_sql(&self) -> Value {
            Value::Text(self.to_owned().into())
        }
    }

    impl ToSql for i64 {
        fn to_sql(&self) -> Value {
            Value::Integer(*self)
        }
    }

    impl ToSql for Integer {
        fn to_sql(&self) -> Value {
            Value::Lit(self.0)
        }
    }

    impl ToSql for Real {
        fn to_sql(&self) -> Value {
            Value::Lit(self.0)
        }
    }

    impl ToSql for Text {
        fn to_sql(&self) -> Value {
            Value::Lit(self.0)
        }
    }

    struct Sql {
        clause: std::sync::Arc<str>,
        params: Vec<Value>,
    }

    macro_rules! sql {
        ($sql:expr, $($args:expr),*) => {{
            let clause = format!($sql, $($args.to_sql(),)*);
            let params = vec![
            $($args.to_sql(),)*
            ].into_iter().filter(|arg| match arg {
                Value::Text(_) => true,
                Value::Real(_) => true,
                Value::Integer(_) => true,
                Value::Blob(_) => true,
                _ => false
            }).collect::<Vec<_>>();

            Sql {
                clause: clause.into(),
                params
            }
        }};
    }

    #[tokio::test]
    async fn sql_macro_works() -> TestResult<()> {
        let accounts = Accounts::new();

        let sql: Sql = sql!("select * from {} where {} = {}", accounts, accounts.id, "1");
        assert_eq!(
            sql.clause.to_string(),
            r#"select * from "accounts" where "accounts"."id" = ?"#
        );
        assert_eq!(sql.params, vec![Value::Text("1".into())]);

        let sql: Sql = sql!("select * from {} where {} = {}", accounts, accounts.id, 1);
        assert_eq!(
            sql.clause.to_string(),
            r#"select * from "accounts" where "accounts"."id" = ?"#
        );
        assert_eq!(sql.params, vec![Value::Integer(1)]);

        Ok(())
    }

    #[tokio::test]
    async fn where_clauses_work() -> TestResult<()> {
        let db = test_db().await?;
        let accounts = Accounts::new();

        let query = db.select(star()).from(accounts).r#where(or(
            and(eq(accounts.id, "1"), eq(accounts.id, "1".to_owned())),
            eq(accounts.id, 1),
        ));

        let sql = query.sql();
        let params = query.values.unwrap();

        assert_eq!(
            sql,
            r#"select * from "accounts" where (("accounts"."id" = ? and "accounts"."id" = ?) or "accounts"."id" = ?)"#
        );
        assert_eq!(
            params,
            vec![
                Value::Text("1".into()),
                Value::Text("1".into()),
                Value::Integer(1)
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn crud_works() -> TestResult<()> {
        let db = test_db().await?;

        let _ = db.execute_batch("create table accounts (id)").await?;

        let accounts = Accounts::new();
        let inserted: Account = db
            .insert(accounts)
            .values(Account { id: 1 })
            .returning(star())
            .await?;

        assert_eq!(inserted.id, 1);

        let new_account = Account { id: 2 };
        let updated: Account = db
            .update(accounts)
            .set(new_account)
            .r#where(eq(accounts.id, 1))
            .returning(star())
            .await?;

        assert_eq!(updated.id, 2);

        let rows: Vec<Account> = db
            .select(star())
            .from(accounts)
            .r#where(eq(accounts.id, 2))
            .all()
            .await?;

        assert_eq!(rows.len(), 1);
        assert_eq!(rows.iter().nth(0).unwrap().id, 2);

        let rows_affected = db
            .delete(accounts)
            .r#where(eq(accounts.id, 2))
            .rows_affected()
            .await?;

        assert_eq!(rows_affected, 1);

        let num_rows: Vec<RowCount> = db
            .select(count(accounts.id))
            .from(accounts)
            .r#where(eq(accounts.id, 2))
            .all()
            .await?;

        assert_ne!(num_rows.iter().nth(0), None);
        assert_eq!(num_rows.iter().nth(0).unwrap().count, 0);

        Ok(())
    }

    #[derive(Row, Deserialize, PartialEq, Debug)]
    struct RowCount {
        count: i64,
    }

    #[derive(Row, Deserialize)]
    struct Account {
        id: i64,
    }

    #[derive(Table, Clone, Copy)]
    #[rizz(table = "accounts")]
    struct Accounts {
        #[rizz(primary_key)]
        id: Integer,
    }
}

#[cfg(not(target_arch = "wasm32"))]
use rusqlite::OpenFlags;
#[cfg(not(target_arch = "wasm32"))]
use serde::de::DeserializeOwned;
#[cfg(not(target_arch = "wasm32"))]
use std::{marker::PhantomData, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
pub fn db(connection: Connection) -> Database {
    Database::new(connection)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn connect(path: &str) -> Result<Connection, Error> {
    Connection::new(path).open().await
}

#[cfg(not(target_arch = "wasm32"))]
pub fn connection(path: &str) -> Connection {
    Connection::new(path)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct Connection {
    path: Arc<str>,
    conn: Option<tokio_rusqlite::Connection>,
    open_flags: OpenFlags,
    pragma: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Connection {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.into(),
            conn: None,
            open_flags: OpenFlags::default(),
            pragma: None,
        }
    }

    pub fn create_if_missing(mut self, arg: bool) -> Self {
        if !arg {
            self.open_flags = self.open_flags.difference(OpenFlags::SQLITE_OPEN_CREATE);
        }
        self
    }

    pub fn read_only(mut self, arg: bool) -> Self {
        if arg == true {
            self.open_flags = self.open_flags.union(OpenFlags::SQLITE_OPEN_READ_ONLY);
        }
        self
    }

    pub fn pragma(mut self, statement: &str) -> Self {
        let s = format!("PRAGMA {};", statement);
        match self.pragma {
            Some(ref mut p) => {
                p.push_str(&s);
            }
            None => {
                self.pragma = Some(s);
            }
        }
        self
    }

    pub fn journal_mode(mut self, mode: JournalMode) -> Self {
        let value = match mode {
            JournalMode::Delete => "DELETE",
            JournalMode::Truncate => "TRUNCATE",
            JournalMode::Persist => "PERSIST",
            JournalMode::Memory => "MEMORY",
            JournalMode::Wal => "WAL",
            JournalMode::Off => "OFF",
        };
        let s = format!("PRAGMA journal_mode = {};", value);
        match self.pragma {
            Some(ref mut p) => {
                p.push_str(&s);
            }
            None => self.pragma = Some(s),
        }
        self
    }

    pub fn synchronous(mut self, sync: Synchronous) -> Self {
        let value = match sync {
            Synchronous::Off => "OFF",
            Synchronous::Normal => "NORMAL",
            Synchronous::Full => "FULL",
            Synchronous::Extra => "EXTRA",
        };
        let s = format!("PRAGMA synchronous = {};", value);
        match self.pragma {
            Some(ref mut p) => {
                p.push_str(&s);
            }
            None => self.pragma = Some(s),
        }
        self
    }

    pub async fn open(mut self) -> Result<Self, Error> {
        let conn = tokio_rusqlite::Connection::open(self.path.as_ref()).await?;
        if let Some(p) = self.pragma.clone() {
            let _ = conn.call(move |conn| conn.execute_batch(&p)).await?;
        }
        self.conn = Some(conn);

        Ok(self)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct Database {
    connection: tokio_rusqlite::Connection,
}

#[cfg(not(target_arch = "wasm32"))]
impl Database {
    fn new(connection: Connection) -> Self {
        Self {
            connection: connection.conn.expect("Database file not found"),
        }
    }

    pub fn select(&self, columns: Arc<str>) -> Query {
        Query::new(self.connection.clone()).select(columns)
    }

    pub fn from(&self, table: impl Table) -> Query {
        Query::new(self.connection.clone()).from(table)
    }

    pub fn insert(&self, table: impl Table) -> Query {
        Query::new(self.connection.clone()).insert(table)
    }

    pub fn update(&self, table: impl Table) -> Query {
        Query::new(self.connection.clone()).update(table)
    }

    pub fn delete(&self, table: impl Table) -> Query {
        Query::new(self.connection.clone()).delete(table)
    }

    pub async fn execute_batch(&self, sql: &str) -> Result<(), Error> {
        let sql = sql.to_owned();
        let _ = self
            .connection
            .call(move |conn| conn.execute_batch(&sql))
            .await?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    #[default]
    Wal,
    Off,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Synchronous {
    Off,
    #[default]
    Normal,
    Full,
    Extra,
}

#[cfg(not(target_arch = "wasm32"))]
impl Value {
    fn to_sql(&self) -> &dyn rusqlite::ToSql {
        match self {
            Value::Text(s) => s,
            Value::Blob(b) => b,
            Value::Real(r) => r,
            Value::Integer(i) => i,
            Value::Lit(s) => s,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn execute(
    connection: &tokio_rusqlite::Connection,
    sql: Arc<str>,
    binds: Option<Vec<Value>>,
) -> Result<usize, Error> {
    let results = connection
        .call(move |conn| {
            let mut stmt = conn.prepare_cached(&sql)?;
            let rows_affected = match binds {
                Some(values) => {
                    let params = values
                        .iter()
                        .map(|value| value.to_sql())
                        .collect::<Vec<_>>();
                    stmt.execute(&*params)?
                }
                None => stmt.execute([])?,
            };
            Ok(rows_affected)
        })
        .await?;

    Ok(results)
}

#[cfg(not(target_arch = "wasm32"))]
async fn rows<T: DeserializeOwned + Send + Sync + 'static>(
    connection: &tokio_rusqlite::Connection,
    sql: Arc<str>,
    binds: Option<Vec<Value>>,
) -> Result<Vec<T>, Error> {
    let results = connection
        .call(move |conn| {
            let mut stmt = conn.prepare_cached(&sql)?;
            let rows = match binds {
                Some(values) => {
                    let params = values
                        .iter()
                        .map(|value| value.to_sql())
                        .collect::<Vec<_>>();
                    stmt.query(&*params)?
                }
                None => stmt.query([])?,
            };
            let rows: Vec<T> = serde_rusqlite::from_rows::<T>(rows)
                .into_iter()
                .filter(|x| x.is_ok())
                .map(|x| x.unwrap())
                .collect();
            Ok(rows)
        })
        .await?;

    Ok(results)
}

#[cfg(not(target_arch = "wasm32"))]
async fn prepare<T: DeserializeOwned + Send + Sync + 'static>(
    connection: &tokio_rusqlite::Connection,
    sql: Arc<str>,
    binds: Option<Vec<Value>>,
) -> Result<Prep<T>, Error> {
    let cloned = connection.clone();
    let prep = connection
        .call(move |conn| {
            // this uses an internal Lru cache within rusqlite
            // and uses the sql as the key to the cache
            // not ideal but what can you do?
            let _ = conn.prepare_cached(&sql)?;
            Ok(Prep {
                connection: cloned,
                sql: sql.into(),
                params: binds,
                phantom: PhantomData::default(),
            })
        })
        .await?;
    Ok(prep)
}

#[cfg(not(target_arch = "wasm32"))]
impl Query {
    fn new(connection: tokio_rusqlite::Connection) -> Self {
        Self {
            connection,
            select: None,
            from: None,
            r#where: None,
            limit: None,
            insert_into: None,
            values_sql: None,
            values: None,
            delete: None,
            set: None,
            update: None,
            returning: None,
        }
    }

    pub fn select(mut self, columns: Arc<str>) -> Self {
        self.select = Some(format!("select {}", columns).into());

        self
    }

    pub fn from(mut self, table: impl Table) -> Self {
        self.from = Some(format!("from {}", table.table_name()).into());

        self
    }

    pub fn r#where(mut self, part: WherePart) -> Self {
        if let None = self.r#where {
            self.r#where = Some(format!("where {}", part.clause).into())
        }
        match self.values {
            Some(ref mut values) => values.extend(part.values),
            None => self.values = Some(part.values),
        }

        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(format!("limit {}", limit).into());
        self
    }

    pub fn sql(&self) -> String {
        self.to_sql().to_string()
    }

    fn to_sql(&self) -> Arc<str> {
        vec![
            self.select.clone(),
            self.from.clone(),
            self.insert_into.clone(),
            self.values_sql.clone(),
            self.update.clone(),
            self.set.clone(),
            self.delete.clone(),
            self.r#where.clone(),
            self.returning.clone(),
            self.limit.clone(),
        ]
        .into_iter()
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .collect::<Vec<_>>()
        .join(" ")
        .into()
    }

    pub async fn all<T: DeserializeOwned + Send + Sync + 'static>(self) -> Result<Vec<T>, Error>
    where
        T: Row,
    {
        let sql = self.sql();
        let rows = rows(&self.connection, sql.into(), self.values).await?;
        Ok(rows)
    }

    pub async fn prepare<T: DeserializeOwned + Send + Sync + 'static>(
        self,
    ) -> Result<Prep<T>, Error>
    where
        T: Row,
    {
        let sql = self.sql();
        let prep = prepare::<T>(&self.connection, sql.into(), self.values).await?;
        Ok(prep)
    }

    pub fn insert(mut self, table: impl Table) -> Self {
        self.insert_into = Some(table.insert_sql().into());
        self
    }

    pub fn values(mut self, row: impl Row) -> Self {
        self.values_sql = Some(row.insert_sql().into());
        self.values = Some(row.values());
        self
    }

    pub fn update(mut self, table: impl Table) -> Self {
        self.update = Some(table.update_sql().into());
        self
    }

    pub fn set(mut self, row: impl Row) -> Self {
        self.set = Some(row.set_sql().into());
        self.values = Some(row.values());
        self
    }

    pub fn delete(mut self, table: impl Table) -> Self {
        self.delete = Some(table.delete_sql().into());
        self
    }

    pub async fn returning<T: Row + DeserializeOwned + Send + Sync + 'static>(
        mut self,
        columns: Arc<str>,
    ) -> Result<T, Error> {
        self.returning = Some(format!("returning {}", columns).into());
        let sql = self.to_sql();
        let rows = rows::<T>(&self.connection, sql.clone(), self.values).await?;
        if let Some(row) = rows.into_iter().nth(0) {
            Ok(row)
        } else {
            Err(Error::InsertError(format!("failed to insert {}", sql)))
        }
    }

    pub async fn rows_affected(self) -> Result<usize, Error> {
        let sql = self.to_sql();
        let rows_affected = execute(&self.connection, sql.clone(), self.values).await?;
        Ok(rows_affected)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct Prep<T>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    connection: tokio_rusqlite::Connection,
    params: Option<Vec<Value>>,
    sql: Arc<str>,
    phantom: PhantomData<T>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> Prep<T>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    async fn all(&self) -> Result<Vec<T>, Error> {
        let rows = rows::<T>(&self.connection, self.sql.clone(), self.params.clone()).await?;
        Ok(rows)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct Query {
    connection: tokio_rusqlite::Connection,
    select: Option<Arc<str>>,
    from: Option<Arc<str>>,
    r#where: Option<Arc<str>>,
    limit: Option<Arc<str>>,
    insert_into: Option<Arc<str>>,
    set: Option<Arc<str>>,
    delete: Option<Arc<str>>,
    values_sql: Option<Arc<str>>,
    returning: Option<Arc<str>>,
    values: Option<Vec<Value>>,
    update: Option<Arc<str>>,
}

#[derive(Clone, Copy)]
pub struct Text(&'static str);

#[derive(Clone, Copy)]
pub struct Blob(&'static str);

#[derive(Clone, Copy)]
pub struct Integer(&'static str);

#[derive(Clone, Copy)]
pub struct Real(&'static str);

pub trait ToColumn {
    fn to_column(&self) -> &'static str;
}

impl ToColumn for Text {
    fn to_column(&self) -> &'static str {
        self.0
    }
}

impl ToColumn for Integer {
    fn to_column(&self) -> &'static str {
        self.0
    }
}

impl ToColumn for Blob {
    fn to_column(&self) -> &'static str {
        self.0
    }
}

impl ToColumn for Real {
    fn to_column(&self) -> &'static str {
        self.0
    }
}

pub fn star() -> std::sync::Arc<str> {
    "*".into()
}

pub fn count(columns: impl ToColumn) -> std::sync::Arc<str> {
    format!("count({}) as count", columns.to_column()).into()
}

pub fn and(left: WherePart, right: WherePart) -> WherePart {
    let mut values: Vec<Value> = vec![];
    values.extend(left.values);
    values.extend(right.values);

    WherePart {
        clause: format!("({} and {})", left.clause, right.clause),
        values,
    }
}

pub fn or(left: WherePart, right: WherePart) -> WherePart {
    let mut values: Vec<Value> = vec![];
    values.extend(left.values);
    values.extend(right.values);

    WherePart {
        clause: format!("({} or {})", left.clause, right.clause),
        values,
    }
}

pub struct WherePart {
    clause: String,
    values: Vec<Value>,
}

pub fn eq(left: impl ToColumn, right: impl Into<Value>) -> WherePart {
    WherePart {
        clause: format!("{} = ?", left.to_column()),
        values: vec![right.into()],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    // TODO: Null,
    Lit(&'static str),
    Text(std::sync::Arc<str>),
    Blob(Vec<u8>),
    Real(f64),
    Integer(i64),
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value.into())
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Text(value.into())
    }
}

impl From<std::sync::Arc<str>> for Value {
    fn from(value: std::sync::Arc<str>) -> Self {
        Value::Text(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Integer(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Real(value)
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Value::Blob(value)
    }
}

pub trait Row {
    fn values(&self) -> Vec<Value>;
    fn insert_sql(&self) -> &'static str;
    fn set_sql(&self) -> &'static str;
}

#[cfg(not(target_arch = "wasm32"))]
pub trait Table {
    fn new() -> Self;
    fn table_name(&self) -> &'static str;
    fn column_names(&self) -> &'static str;
    fn insert_sql(&self) -> &'static str;
    fn update_sql(&self) -> &'static str;
    fn delete_sql(&self) -> &'static str;
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database connection closed")]
    ConnectionClosed,
    #[error("database connection closing: {0}")]
    Close(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("missing from statement in sql query")]
    MissingFrom,
    #[error("error inserting record {0}")]
    InsertError(String),
}

#[cfg(not(target_arch = "wasm32"))]
impl From<tokio_rusqlite::Error> for Error {
    fn from(value: tokio_rusqlite::Error) -> Self {
        match value {
            tokio_rusqlite::Error::ConnectionClosed => Self::ConnectionClosed,
            tokio_rusqlite::Error::Close((_, error)) => Self::Close(error.to_string()),
            tokio_rusqlite::Error::Rusqlite(err) => Self::Database(err.to_string()),
            _ => todo!(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            rusqlite::Error::SqliteFailure(_, _) => todo!(),
            rusqlite::Error::SqliteSingleThreadedMode => todo!(),
            rusqlite::Error::FromSqlConversionFailure(_, _, _) => todo!(),
            rusqlite::Error::IntegralValueOutOfRange(_, _) => todo!(),
            rusqlite::Error::Utf8Error(_) => todo!(),
            rusqlite::Error::NulError(_) => todo!(),
            rusqlite::Error::InvalidParameterName(_) => todo!(),
            rusqlite::Error::InvalidPath(_) => todo!(),
            rusqlite::Error::ExecuteReturnedResults => todo!(),
            rusqlite::Error::QueryReturnedNoRows => todo!(),
            rusqlite::Error::InvalidColumnIndex(_) => todo!(),
            rusqlite::Error::InvalidColumnName(_) => todo!(),
            rusqlite::Error::InvalidColumnType(_, _, _) => todo!(),
            rusqlite::Error::StatementChangedRows(_) => todo!(),
            rusqlite::Error::ToSqlConversionFailure(_) => todo!(),
            rusqlite::Error::InvalidQuery => todo!(),
            rusqlite::Error::MultipleStatement => todo!(),
            rusqlite::Error::InvalidParameterCount(_, _) => todo!(),
            _ => todo!(),
        }
    }
}
