use std::future::Future;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::model::*;

const TEST_TABLE_ID: TableId = TableId(1);
const CREATED_AT: DateTime<Utc> = DateTime::<Utc>::UNIX_EPOCH;
const FORECAST_READY_AT: DateTime<Utc> = DateTime::<Utc>::UNIX_EPOCH;

fn test_new_item() -> NewItem {
    NewItem {
        name: "test new item".into(),
        comment: "test new item comment".into(),
        created_at: CREATED_AT,
        forecast_ready_at: FORECAST_READY_AT,
    }
}

fn test_new_item_2() -> NewItem {
    NewItem {
        name: "test other item".into(),
        comment: "test other item comment".into(),
        created_at: CREATED_AT,
        forecast_ready_at: FORECAST_READY_AT,
    }
}

#[async_trait]
pub trait StorageBuilder<S: Storage> {
    async fn build(&self) -> S;
}

#[async_trait]
impl<S, F, Fu> StorageBuilder<S> for F
where
    S: Storage,
    Fu: Future<Output = S> + Send,
    F: Fn() -> Fu,
    F: Send + Sync,
{
    async fn build(&self) -> S {
        self().await
    }
}

pub fn test_suite<S>(builder: impl StorageBuilder<S>) -> Result<(), S::Error>
where
    S: Storage,
{
    run_test(&builder, initially_empty)?;
    run_test(&builder, add_single)?;
    run_test(&builder, add_multiple)?;
    run_test(&builder, list_twice)?;
    run_test(&builder, get_twice)?;
    run_test(&builder, add_remove_single)?;
    run_test(&builder, add_remove_multiple)?;
    run_test(&builder, remove_nonexistent)?;
    run_test(&builder, remove_mixed)?;

    Ok(())
}

async fn initially_empty<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());
    Ok(())
}

async fn add_single<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();
    s.add_items(TEST_TABLE_ID, [item.clone()].into_iter())
        .await?;

    let items = s.list_items(TEST_TABLE_ID).await?;
    assert!(matches!(
        items[..],
        [ItemInfoShort {
            table_id: TEST_TABLE_ID,
            ref name,
            ..
        }]
        if name == &item.name
    ));

    let roundtrip_item = s.get_item(TEST_TABLE_ID, items[0].item_id.clone()).await?;
    assert!(matches!(
        roundtrip_item,
        Some(ItemInfo {
            table_id: TEST_TABLE_ID,
            ref name,
            ref created_at,
            ref forecast_ready_at,
            ..
        })
        if name == &item.name && created_at == &item.created_at && forecast_ready_at == &item.forecast_ready_at
    ));

    Ok(())
}

async fn add_multiple<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();
    let item2 = test_new_item_2();

    s.add_items(TEST_TABLE_ID, [item.clone(), item2.clone()].into_iter())
        .await?;

    let items = s.list_items(TEST_TABLE_ID).await?;
    assert!(matches!(
        items[..],
        [
            ItemInfoShort {
                table_id: TEST_TABLE_ID,
                name: ref name1,
                ..
            },
            ItemInfoShort {
                table_id: TEST_TABLE_ID,
                name: ref name2,
                ..
            },
        ]
        if name1 == &item.name && name2 == &item2.name
    ));

    let roundtrip_item = s.get_item(TEST_TABLE_ID, items[1].item_id.clone()).await?;
    assert!(matches!(
        roundtrip_item,
        Some(ItemInfo {
            table_id: TEST_TABLE_ID,
            ref name,
            ref created_at,
            ref forecast_ready_at,
            ..
        })
        if name == &item2.name && created_at == &item2.created_at && forecast_ready_at == &item2.forecast_ready_at
    ));

    Ok(())
}

async fn list_twice<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();
    let item2 = test_new_item_2();

    s.add_items(TEST_TABLE_ID, [item.clone(), item2.clone()].into_iter())
        .await?;

    let items = s.list_items(TEST_TABLE_ID).await?;
    let second_items = s.list_items(TEST_TABLE_ID).await?;
    assert_eq!(items, second_items);

    Ok(())
}

async fn get_twice<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();
    let item2 = test_new_item_2();

    s.add_items(TEST_TABLE_ID, [item.clone(), item2.clone()].into_iter())
        .await?;

    let items = s.list_items(TEST_TABLE_ID).await?;

    let first_item = s.get_item(TEST_TABLE_ID, items[0].item_id.clone()).await?;
    let second_item = s.get_item(TEST_TABLE_ID, items[0].item_id.clone()).await?;
    assert_eq!(first_item, second_item);

    Ok(())
}

async fn add_remove_single<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();

    s.add_items(TEST_TABLE_ID, [item.clone()].into_iter())
        .await?;
    let items = s.list_items(TEST_TABLE_ID).await?;
    let item_id = items[0].item_id.clone();
    s.remove_items(TEST_TABLE_ID, [item_id.clone()].into_iter())
        .await?;

    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());
    assert_eq!(s.get_item(TEST_TABLE_ID, item_id).await?, None);

    Ok(())
}

async fn add_remove_multiple<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();
    let item2 = test_new_item_2();

    s.add_items(TEST_TABLE_ID, [item.clone(), item2.clone()].into_iter())
        .await?;
    let items = s.list_items(TEST_TABLE_ID).await?;
    let item_id = items[0].item_id.clone();
    let item2_id = items[1].item_id.clone();
    s.remove_items(
        TEST_TABLE_ID,
        [item_id.clone(), item2_id.clone()].into_iter(),
    )
    .await?;

    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());
    assert_eq!(s.get_item(TEST_TABLE_ID, item_id).await?, None);
    assert_eq!(s.get_item(TEST_TABLE_ID, item2_id).await?, None);

    Ok(())
}

async fn remove_nonexistent<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item_id: ItemId = 0.into();

    s.remove_items(TEST_TABLE_ID, [item_id.clone()].into_iter())
        .await?;

    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());
    assert_eq!(s.get_item(TEST_TABLE_ID, item_id).await?, None);

    Ok(())
}

async fn remove_mixed<S>(s: S) -> Result<(), S::Error>
where
    S: Storage,
{
    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());

    let item = test_new_item();

    s.add_items(TEST_TABLE_ID, [item.clone()].into_iter())
        .await?;
    let items = s.list_items(TEST_TABLE_ID).await?;
    let item_id = items[0].item_id.clone();
    let missing_item_id: ItemId = if item_id == 0.into() {
        1.into()
    } else {
        0.into()
    };
    s.remove_items(
        TEST_TABLE_ID,
        [item_id.clone(), missing_item_id.clone()].into_iter(),
    )
    .await?;

    assert!(s.list_items(TEST_TABLE_ID).await?.is_empty());
    assert_eq!(s.get_item(TEST_TABLE_ID, item_id).await?, None);

    Ok(())
}

fn run_test<S, Fut, TestFn>(
    builder: &impl StorageBuilder<S>,
    test_fn: TestFn,
) -> Result<(), S::Error>
where
    S: Storage,
    Fut: Future<Output = Result<(), S::Error>>,
    TestFn: Fn(S) -> Fut,
{
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let storage = builder.build().await;
        test_fn(storage).await
    })?;
    Ok(())
}
