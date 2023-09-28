use chrono::{DateTime, Utc};
use derive_more::From;

#[derive(Debug, Eq, PartialEq, Hash, Clone, From)]
pub struct TableId(i32);

#[derive(Debug, Eq, PartialEq, Hash, Clone, From)]
pub struct ItemId(i32);

#[derive(Clone)]
pub struct NewItem {
    pub name: String,
    pub comment: String,
    pub created_at: DateTime<Utc>,
    pub forecast_ready_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemInfoShort {
    pub table_id: TableId,
    pub item_id: ItemId,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemInfo {
    pub table_id: TableId,
    pub item_id: ItemId,
    pub name: String,
    pub comment: String,
    pub created_at: DateTime<Utc>,
    pub forecast_ready_at: DateTime<Utc>,
}
