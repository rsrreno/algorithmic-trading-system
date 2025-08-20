use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use tracing::info;

pub type Database = Pool<Sqlite>;

pub async fn init(database_url: &str) -> Result<Database> {
    info!("Connecting to database: {}", database_url);
    
    // Create database file if it doesn't exist (for SQLite)
    if database_url.starts_with("sqlite:") {
        let path = database_url.strip_prefix("sqlite:").unwrap_or(database_url);
        
        // For relative paths, make sure we can create the database
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
                info!("Created database directory: {:?}", parent);
            }
        }
        
        // Try to create the file if it doesn't exist
        if !std::path::Path::new(path).exists() {
            std::fs::File::create(path)?;
            info!("Created database file: {}", path);
        }
    }
    
    let pool = SqlitePool::connect(database_url).await?;
    
    info!("Database connection established");
    Ok(pool)
}

pub async fn migrate(db: &Database) -> Result<()> {
    info!("Running database migrations");
    
    sqlx::migrate!("./migrations").run(db).await?;
    
    info!("Database migrations completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_init() {
        let db = init("sqlite::memory:").await.unwrap();
        migrate(&db).await.unwrap();
    }
}