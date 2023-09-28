use std::collections::HashMap;
use std::convert::Infallible;
use std::ops::RangeFrom;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::instrument;

use super::model::*;

struct SimpleMemoryStorageInner {
    item_id_seq: RangeFrom<i32>,
    items: HashMap<TableId, Vec<ItemInfo>>,
}

impl Default for SimpleMemoryStorageInner {
    fn default() -> Self {
        SimpleMemoryStorageInner {
            item_id_seq: 0..,
            items: Default::default(),
        }
    }
}

impl SimpleMemoryStorageInner {
    fn add_items(&mut self, table_id: TableId, items: impl Iterator<Item = NewItem> + Send) {
        let mut generate_item_id = || -> ItemId {
            self.item_id_seq
                .next()
                .expect("Item ids sequence overflow")
                .into()
        };

        self.items
            .entry(table_id.clone())
            .or_insert(vec![])
            .extend(items.map(|i| ItemInfo {
                table_id: table_id.clone(),
                item_id: generate_item_id(),
                name: i.name,
                comment: i.comment,
                created_at: i.created_at,
                forecast_ready_at: i.forecast_ready_at,
            }));
    }
}

#[derive(Default)]
pub struct SimpleMemoryStorage {
    inner: Mutex<SimpleMemoryStorageInner>,
}

type SimpleMemoryStorageError = Infallible;

#[async_trait]
impl Storage for SimpleMemoryStorage {
    type Error = SimpleMemoryStorageError;

    #[instrument(skip(self, items))]
    async fn add_items(
        &self,
        table_id: TableId,
        items: impl Iterator<Item = NewItem> + Send,
    ) -> Result<(), Self::Error> {
        let mut data = self.inner.lock().await;
        data.add_items(table_id, items);
        Ok(())
    }

    #[instrument(skip(self, item_ids))]
    async fn remove_items(
        &self,
        table_id: TableId,
        item_ids: impl Iterator<Item = ItemId> + Send,
    ) -> Result<(), Self::Error> {
        let mut data = self.inner.lock().await;

        // TODO collect to Set?
        // TODO Use one more map in data and remove .collect() at all?
        let item_ids = item_ids.collect::<Vec<_>>();

        // We can leave table entry in map, assuming there's a cap on total tables in storage
        if let Some(table_items) = data.items.get_mut(&table_id) {
            table_items.retain(|i| !item_ids.contains(&i.item_id));
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_items(&self, table_id: TableId) -> Result<Vec<ItemInfoShort>, Self::Error> {
        let mut data = self.inner.lock().await;

        Ok(data
            .items
            .get_mut(&table_id)
            .map(|items| {
                items
                    .iter()
                    .map(|item| ItemInfoShort {
                        table_id: item.table_id.clone(),
                        item_id: item.item_id.clone(),
                        name: item.name.clone(),
                    })
                    .collect()
            })
            .unwrap_or(vec![]))
    }

    #[instrument(skip(self))]
    async fn get_item(
        &self,
        table_id: TableId,
        item_id: ItemId,
    ) -> Result<Option<ItemInfo>, Self::Error> {
        let mut data = self.inner.lock().await;

        Ok(data
            .items
            .get_mut(&table_id)
            .map(|items| items.iter().find(|item| item.item_id == item_id).cloned())
            .unwrap_or(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::storage::testing::test_suite;

    #[test]
    fn test_memory_storage() {
        test_suite(&|| async { SimpleMemoryStorage::default() }).unwrap()
    }
}
