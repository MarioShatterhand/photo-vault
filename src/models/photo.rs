use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Photo {
    pub id: i64,
    pub filename: String,
    pub original_name: String,
    pub hash: String,
    pub size: i64,
    pub created_at: String,
}
