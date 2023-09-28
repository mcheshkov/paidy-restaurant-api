use async_trait::async_trait;
use deadpool_postgres::{Client as PoolClient, Pool, PoolError};
use derive_more::From;
use thiserror::Error;
use tokio_postgres::{
    types::FromSql, Client, Column, Error as PgError, IsolationLevel, Row, Transaction,
};
use tracing::instrument;

use super::model::*;

#[derive(Debug, Error, From)]
pub enum PostgresStorageError {
    #[error(transparent)]
    PoolError(PoolError),
    #[error(transparent)]
    DbError(PgError),
    #[error("column `{0}` not found in result set, this is most probably a bug, mismatch between query and parser")]
    #[from(ignore)]
    ColumnNotFound(&'static str),
}

/// Generic interface to parse result sets from DB to Rust types
/// Could be implemented manually, or via `rows_parser_struct` macro
trait RowsParser: Sized {
    type Output;

    fn prepare(columns: &[Column]) -> Result<Self, PostgresStorageError>;
    fn parse(&self, row: Row) -> Result<Self::Output, PostgresStorageError>;
    fn parse_one(row: Row) -> Result<Self::Output, PostgresStorageError> {
        let parser = Self::prepare(row.columns())?;
        parser.parse(row)
    }
    fn parse_many(rows: Vec<Row>) -> Result<Vec<Self::Output>, PostgresStorageError> {
        if rows.is_empty() {
            return Ok(vec![]);
        }

        let parser = Self::prepare(rows[0].columns())?;
        rows.into_iter().map(|row| parser.parse(row)).collect()
    }
}

// This could probably be much better with derive macro

/// Macro to generate type implementing RowsParser
/// Generated type will build column map in `prepare`,
/// and the use it in parse call to avoid looking up column idx by name for every row
macro_rules! rows_parser_struct {
    ($ty: ident, $out_ty: ident, $(($field: ident, $column: literal, $($native_ty: ident)?),)+) => (
        struct $ty {
            columns_map: [usize; rows_parser_struct!(@len, $($column,)+)],
        }
        impl RowsParser for $ty {
            type Output = $out_ty;
            fn prepare(columns: &[Column]) -> Result<Self, PostgresStorageError> {
                let columns_map = rows_parser_struct!(@columns, columns, $($column,)+);
                Ok(Self { columns_map })
            }
            fn parse(&self, row: Row) -> Result<Self::Output, PostgresStorageError> {
                rows_parser_struct!(@fields, row, self.columns_map, 0, $(($field, $($native_ty)?), )+);
                Ok($out_ty {
                    $($field,)+
                })
            }
        }
    );
    (@len, $column: literal,) => (
        1
    );
    (@len, $column: literal, $($rest_column: literal,)+) => (
        1+rows_parser_struct!(@len, $($rest_column,)+)
    );
    (@columns, $columns: ident, $($column: literal,)+) => ({
        const COLUMNS: &[&'static str; rows_parser_struct!(@len, $($column,)+)] = &[$($column,)+];
        PostgresStorage::build_column_map(&COLUMNS, &$columns)?
    });
    (@fields, $row: ident, $columns_map: expr, $idx: expr, ($field: ident, $($native_ty: ident)?),) => (
        rows_parser_struct!(@single_field, $row, $columns_map, $idx, $field, $($native_ty)?);
    );
    (@fields, $row: ident, $columns_map: expr, $idx: expr, ($field: ident, $($native_ty: ident)?), $(($rest_field: ident, $($rest_native_ty: ident)?),)+) => (
        rows_parser_struct!(@single_field, $row, $columns_map, $idx, $field, $($native_ty)?);
        rows_parser_struct!(@fields, $row, $columns_map, $idx+1, $(($rest_field,$($rest_native_ty)?),)+);
    );
    (@single_field, $row: ident, $columns_map: expr, $idx: expr, $field: ident,) => (
        let $field = PostgresStorage::try_get_field(&$row, $columns_map[$idx])?;
    );
    (@single_field, $row: ident, $columns_map: expr, $idx: expr, $field: ident, $native_ty: ident) => (
        let $field = PostgresStorage::try_get_field::<$native_ty>(&$row, $columns_map[$idx])?.into();
    );
}

rows_parser_struct!(
    ItemInfoShortParser,
    ItemInfoShort,
    (table_id, "table_id", i32),
    (item_id, "item_id", i32),
    (name, "name",),
);

rows_parser_struct!(
    ItemInfoParser,
    ItemInfo,
    (table_id, "table_id", i32),
    (item_id, "item_id", i32),
    (name, "name",),
    (comment, "comment",),
    (created_at, "created_at",),
    (forecast_ready_at, "forecast_ready_at",),
);

pub struct PostgresStorage {
    pool: Pool,
}

impl PostgresStorage {
    pub fn new(pool: Pool) -> PostgresStorage {
        PostgresStorage { pool }
    }

    fn try_get_field<T: for<'a> FromSql<'a>>(
        row: &Row,
        name: usize,
    ) -> Result<T, PostgresStorageError> {
        Ok(row.try_get(name)?)
    }

    async fn get_db_client(&self) -> Result<PoolClient, PostgresStorageError> {
        Ok(self.pool.get().await?)
    }

    async fn start_transaction(db: &mut Client) -> Result<Transaction, PostgresStorageError> {
        Ok(db
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .await?)
    }

    async fn start_readonly_transaction(
        db: &mut Client,
    ) -> Result<Transaction, PostgresStorageError> {
        Ok(db
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .read_only(true)
            .start()
            .await?)
    }

    fn build_column_map<const N: usize>(
        reference_columns: &[&'static str; N],
        input_columns: &[Column],
    ) -> Result<[usize; N], PostgresStorageError> {
        // array::try_map is unstable
        let mut result: [usize; N] = [0; N];
        for (reference_idx, reference_column) in reference_columns.iter().enumerate() {
            let input_idx = input_columns
                .iter()
                .position(|input_column| input_column.name() == *reference_column)
                // We could use Error::column from tokio_postgres here, but it's private
                .ok_or_else(|| PostgresStorageError::ColumnNotFound(reference_column))?;
            result[reference_idx] = input_idx;
        }
        Ok(result)
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    type Error = PostgresStorageError;

    #[instrument(skip(self, items))]
    async fn add_items(
        &self,
        table_id: TableId,
        items: impl Iterator<Item = NewItem> + Send,
    ) -> Result<(), Self::Error> {
        let mut db = self.get_db_client().await?;
        let txn = Self::start_transaction(&mut db).await?;

        // TODO use pipelining here, but carefully, to avoid out-of-order item ids
        // could use futures::stream::Stream here, but decided to keep it simple for now
        for item in items {
            txn.execute(
                // language=PostgreSQL
                "
                INSERT INTO
                    items
                    (table_id, name, comment, created_at, forecast_ready_at)
                VALUES
                    ($1, $2, $3, $4, $5)
            ",
                &[
                    &(table_id.0),
                    &item.name,
                    &item.comment,
                    &item.created_at,
                    &item.forecast_ready_at,
                ],
            )
            .await?;
        }

        txn.commit().await?;

        Ok(())
    }

    #[instrument(skip(self, item_ids))]
    async fn remove_items(
        &self,
        table_id: TableId,
        item_ids: impl Iterator<Item = ItemId> + Send,
    ) -> Result<(), Self::Error> {
        let mut db = self.get_db_client().await?;
        let txn = Self::start_transaction(&mut db).await?;

        let item_ids = item_ids.map(|id| id.0).collect::<Vec<_>>();

        txn.execute(
            // language=PostgreSQL
            "
                    DELETE FROM
                        items
                    WHERE
                        table_id = $1
                        AND
                        item_id = ANY($2)
                ",
            &[&table_id.0, &item_ids],
        )
        .await?;

        txn.commit().await?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_items(&self, table_id: TableId) -> Result<Vec<ItemInfoShort>, Self::Error> {
        let mut db = self.get_db_client().await?;
        let txn = Self::start_readonly_transaction(&mut db).await?;

        let rows = txn
            .query(
                // language=PostgreSQL
                "
                    SELECT
                        table_id,
                        item_id,
                        name
                    FROM
                        items
                    WHERE
                        table_id = $1
                    ORDER BY
                        item_id
                ",
                &[&table_id.0],
            )
            .await?;

        txn.commit().await?;

        ItemInfoShortParser::parse_many(rows)
    }

    #[instrument(skip(self))]
    async fn get_item(
        &self,
        table_id: TableId,
        item_id: ItemId,
    ) -> Result<Option<ItemInfo>, Self::Error> {
        let mut db = self.get_db_client().await?;
        let txn = Self::start_readonly_transaction(&mut db).await?;

        let row = txn
            .query_opt(
                // language=PostgreSQL
                "
                    SELECT
                        table_id,
                        item_id,
                        name,
                        comment,
                        created_at,
                        forecast_ready_at
                    FROM
                        items
                    WHERE
                        table_id = $1
                        AND
                        item_id = $2
                ",
                &[&table_id.0, &item_id.0],
            )
            .await?;

        txn.commit().await?;

        row.map(ItemInfoParser::parse_one).transpose()
    }
}
