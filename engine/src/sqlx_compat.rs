pub use sqlx_core::error::Error;
pub use sqlx_core::query::query;
pub use sqlx_core::query_builder::QueryBuilder;
pub use sqlx_core::row::Row;
pub use sqlx_core::types;
pub use sqlx_postgres::{PgPool, PgPoolOptions, Postgres};

#[allow(unused_imports)]
pub mod postgres {
    pub use sqlx_postgres::types;
    pub use sqlx_postgres::{self, PgPool, PgPoolOptions, PgRow, PgSslMode, Postgres};
}
