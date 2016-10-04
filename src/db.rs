use r2d2;
use r2d2_postgres::{PostgresConnectionManager};

pub type PostgresConnection = r2d2::PooledConnection<PostgresConnectionManager>;
