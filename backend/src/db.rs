use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create database pool");

    // Run pending migrations automatically
    let mut conn = pool.get().expect("Failed to get database connection");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    log::info!("Database migrations completed successfully");

    pool
}
