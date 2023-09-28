use async_trait::async_trait;
use chrono::{DateTime, Utc};
use derive_more::From;

#[derive(Debug, Eq, PartialEq, Hash, Clone, From)]
pub struct TableId(pub(super) i32);

#[derive(Debug, Eq, PartialEq, Hash, Clone, From)]
pub struct ItemId(pub(super) i32);

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

/// Everything that is needed to persist data
/// Each method represents atomic operation from the storage PoV
/// Implementation should guarantee data safety on cancellation: dropped futures can leave
/// storage in state either before or after transaction, but must not leave it halfway,
/// nor in other unusable/broken/inconsistent state.
#[async_trait]
pub trait Storage {
    type Error: std::error::Error;

    /// Adds new items to table. Table id is not validated.
    /// Should generate unique item id for each new item.
    async fn add_items(
        &self,
        table_id: TableId,
        items: impl Iterator<Item = NewItem> + Send,
    ) -> Result<(), Self::Error>;

    /// Removes items from table. Table id is not validated.
    /// Should skip over item ids not present on table.
    async fn remove_items(
        &self,
        table_id: TableId,
        item_ids: impl Iterator<Item = ItemId> + Send,
    ) -> Result<(), Self::Error>;

    /// List all items for a table.
    /// Should preserve order of elements:
    /// * if two elements were added in same add_items call they should appear in same order
    /// * if tow elements were added in different add_items calls, but one has finished before
    /// other started then order of items should be same as order of add_items calls
    async fn list_items(&self, table_id: TableId) -> Result<Vec<ItemInfoShort>, Self::Error>;

    /// Get single item
    /// TableId is not really necessary here, but by having it we can allow for storage
    /// to include TableId to item primary key
    async fn get_item(
        &self,
        table_id: TableId,
        item_id: ItemId,
    ) -> Result<Option<ItemInfo>, Self::Error>;
}
