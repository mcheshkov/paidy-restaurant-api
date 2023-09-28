mod service;
mod storage;

use std::sync::Arc;

use anyhow::anyhow;
use clap::Parser;
use deadpool_postgres::PoolConfig;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::service::{DefaultRestaurantService, RestaurantService};
use crate::storage::pg::PostgresStorage;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Postgres host
    #[arg(long)]
    postgres_host: String,

    /// Postgres host
    #[arg(long, default_value_t = 5432)]
    postgres_port: u16,

    /// Postgres host
    #[arg(long, env)]
    postgres_username: String,

    /// Postgres host
    #[arg(long, env)]
    postgres_password: String,

    /// Postgres host
    #[arg(long)]
    postgres_database: String,

    /// Postgres connections pool size
    #[arg(long)]
    postgres_pool: usize,

    /// Count of load generating tasks
    #[arg(long)]
    tasks: usize,

    // This should be separate migrator executable
    /// Run DB initialization and exit
    #[arg(long, default_value_t = false)]
    init_and_exit: bool,
}

async fn load_simulator_task<S>(service: Arc<S>, token: CancellationToken) -> anyhow::Result<()>
where
    S: RestaurantService,
    S::Error: Send + Sync + 'static,
{
    use std::collections::HashSet;

    use rand::distributions::{Distribution, Standard};
    use rand::seq::IteratorRandom;
    use rand::Rng;

    use crate::service::NewItem;
    use crate::storage::model::TableId;

    let mut known_item_ids = HashSet::new();

    loop {
        // Issuing queries non-stop, there's no need to use Future + select
        if token.is_cancelled() {
            info!("Received stop signal");
            break;
        }

        enum Op {
            Add,
            Remove,
            List,
            Get,
        }

        // TODO should probably be Uniform instead of Standard
        impl Distribution<Op> for Standard {
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Op {
                match rng.gen_range(0..=3) {
                    0 => Op::Add,
                    1 => Op::Remove,
                    2 => Op::List,
                    _ => Op::Get,
                }
            }
        }

        fn gen_table_id(rng: &mut impl Rng) -> TableId {
            rng.gen_range(0..10).into()
        }

        let op: Op = {
            let mut rng = rand::thread_rng();
            rng.gen()
        };

        match op {
            Op::Add => {
                let (table_id, items) = {
                    let mut rng = rand::thread_rng();
                    let table_id = gen_table_id(&mut rng);
                    let item_count = rng.gen_range(0..10);
                    let items = (0..item_count).map(|_| NewItem {
                        name: "new item".into(),
                        comment: "".into(),
                    });
                    (table_id, items)
                };

                service.add_items(table_id, items.into_iter()).await?;
            }
            Op::Remove => {
                let (table_id, item_ids) = {
                    let mut rng = rand::thread_rng();
                    let table_id = gen_table_id(&mut rng);
                    let item_count = rng.gen_range(0..10);
                    let item_ids = known_item_ids
                        .iter()
                        .cloned()
                        .choose_multiple(&mut rng, item_count);
                    (table_id, item_ids)
                };
                for item_id in item_ids.iter() {
                    // This would remove too much, but only until next list_items
                    known_item_ids.remove(item_id);
                }
                service.remove_items(table_id, item_ids.into_iter()).await?;
            }
            Op::List => {
                let table_id = {
                    let mut rng = rand::thread_rng();
                    gen_table_id(&mut rng)
                };
                let items = service.list_items(table_id).await?;
                known_item_ids.extend(items.into_iter().map(|i| i.item_id));
            }
            Op::Get => {
                // This would have very low hit rate
                let (table_id, item_id) = {
                    let mut rng = rand::thread_rng();
                    let table_id = gen_table_id(&mut rng);
                    let item_id = known_item_ids.iter().cloned().choose(&mut rng);
                    let item_id = match item_id {
                        None => continue,
                        Some(item_id) => item_id,
                    };
                    (table_id, item_id)
                };
                service.get_item(table_id, item_id).await?;
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    {
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};
        tracing_subscriber::registry()
            .with(fmt::layer().pretty())
            .with(EnvFilter::from_default_env())
            .init();
    }

    let args = Args::parse();

    let pool = {
        use deadpool_postgres::tokio_postgres::NoTls;
        use deadpool_postgres::Config;

        let mut cfg = Config::new();
        cfg.host = Some(args.postgres_host);
        cfg.port = Some(args.postgres_port);
        cfg.user = Some(args.postgres_username);
        cfg.password = Some(args.postgres_password);
        cfg.dbname = Some(args.postgres_database);
        cfg.pool = Some(PoolConfig {
            max_size: args.postgres_pool,
            ..Default::default()
        });
        let pool = cfg.create_pool(None, NoTls)?;
        {
            // Just to check connectivity
            let db = pool.get().await?;
            drop(db);
        }
        pool
    };

    if args.init_and_exit {
        storage::pg::init_db(&pool).await?;
        return Ok(());
    }

    let storage = PostgresStorage::new(pool);
    // let storage = storage::SimpleMemoryStorage::default();
    let service = DefaultRestaurantService::new(storage);
    let service = Arc::new(service);

    let cancellation = CancellationToken::new();
    let mut set = JoinSet::new();

    for _ in 0..args.tasks {
        let service = service.clone();
        let token = cancellation.child_token();
        set.spawn(load_simulator_task(service, token));
    }

    let mut result = Ok(());

    let interrupt = tokio::signal::ctrl_c();
    tokio::select! {
        _ = interrupt => {
            info!("Interrupted, shutting down");
        },
        r = set.join_next() => {
            error!(result=?r, "Task stopped unexpectedly");
            result = Err(anyhow!("Task stopped unexpectedly"));
        },
    }

    cancellation.cancel();

    while let Some(res) = set.join_next().await {
        match res {
            Err(e) => {
                error!(error=?e, "Task joined with error");
                result = Err(anyhow!(e));
            }
            Ok(Err(e)) => {
                error!(error=?e, "Task finished with error");
                result = Err(anyhow!(e));
            }
            Ok(Ok(_)) => {
                // Do nothing
            }
        }
    }

    result
}
