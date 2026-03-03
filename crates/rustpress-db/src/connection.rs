use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tracing::info;

/// Connect to the WordPress database using SeaORM.
pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opts = ConnectOptions::new(database_url);
    opts.max_connections(10)
        .min_connections(1)
        .sqlx_logging(true);

    info!("Connecting to database...");
    let db = Database::connect(opts).await?;
    info!("Database connection established");

    Ok(db)
}
