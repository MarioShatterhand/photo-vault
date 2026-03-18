use dioxus::prelude::*;
use crate::models::Photo;

#[get("/api/list-photos")]
pub async fn list_photos() -> Result<Vec<Photo>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let photos = sqlx::query_as::<_, Photo>(
            "SELECT * FROM photos ORDER BY created_at DESC"
        )
        .fetch_all(&*crate::server::db::DB)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(photos);
    }
    #[cfg(not(feature = "server"))]
    unreachable!("This function should not be called directly on the client")
}
