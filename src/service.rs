use std::fmt::Debug;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use derive_more::From;
use thiserror::Error;
use tracing::instrument;

use crate::storage::model::{
    ItemId, ItemInfo, ItemInfoShort, NewItem as StorageNewItem, Storage, TableId,
};

pub struct NewItem {
    pub name: String,
    pub comment: String,
}

/// Service implementation: this is the reflection of public service API in Rust
/// It may or may not use Storage to actually persist any items.
/// It may represent HTTP client as well as service implementation.
#[async_trait]
pub trait RestaurantService {
    type Error: std::error::Error;

    async fn add_items(
        &self,
        table_id: TableId,
        items: impl Iterator<Item = NewItem> + Send,
    ) -> Result<(), Self::Error>;

    async fn remove_items(
        &self,
        table_id: TableId,
        item_ids: impl Iterator<Item = ItemId> + Send,
    ) -> Result<(), Self::Error>;

    async fn list_items(&self, table_id: TableId) -> Result<Vec<ItemInfoShort>, Self::Error>;

    async fn get_item(
        &self,
        table_id: TableId,
        item_id: ItemId,
    ) -> Result<Option<ItemInfo>, Self::Error>;
}

#[derive(Debug, Error, From)]
pub enum DefaultRestaurantServiceError<SE: std::error::Error> {
    #[error(transparent)]
    StorageError(SE),
}

pub struct DefaultRestaurantService<S> {
    storage: S,
}

impl<S> DefaultRestaurantService<S> {
    pub fn new(storage: S) -> DefaultRestaurantService<S> {
        DefaultRestaurantService { storage }
    }

    fn get_forecast() -> Duration {
        // TODO either lift RNG instance up (e.g. to struct fields) or use proper forecasting
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let seconds = rng.gen_range(5 * 60..15 * 60);
        Duration::seconds(seconds)
    }
}

#[async_trait]
impl<S: Storage + Send + Sync> RestaurantService for DefaultRestaurantService<S> {
    type Error = DefaultRestaurantServiceError<S::Error>;

    #[instrument(skip(self, items))]
    async fn add_items(
        &self,
        table_id: TableId,
        items: impl Iterator<Item = NewItem> + Send,
    ) -> Result<(), Self::Error> {
        let now = Utc::now();
        Ok(self
            .storage
            .add_items(
                table_id,
                items.map(|i| StorageNewItem {
                    name: i.name,
                    comment: i.comment,
                    created_at: now,
                    forecast_ready_at: now + Self::get_forecast(),
                }),
            )
            .await?)
    }

    #[instrument(skip(self, item_ids))]
    async fn remove_items(
        &self,
        table_id: TableId,
        item_ids: impl Iterator<Item = ItemId> + Send,
    ) -> Result<(), Self::Error> {
        Ok(self.storage.remove_items(table_id, item_ids).await?)
    }

    #[instrument(skip(self))]
    async fn list_items(&self, table_id: TableId) -> Result<Vec<ItemInfoShort>, Self::Error> {
        Ok(self.storage.list_items(table_id).await?)
    }

    #[instrument(skip(self))]
    async fn get_item(
        &self,
        table_id: TableId,
        item_id: ItemId,
    ) -> Result<Option<ItemInfo>, Self::Error> {
        Ok(self.storage.get_item(table_id, item_id).await?)
    }
}
