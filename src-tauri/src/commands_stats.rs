use crate::models::Profile;
use crate::services::analytics_service::{self, Stats, StatsRangeInput};
use crate::ChatnipService;
use tauri::State;

#[tauri::command]
pub async fn get_stats(service: State<'_, ChatnipService>, input: StatsRangeInput) -> Result<Stats, String> {
    let profile = Profile::get_active(service.db())
        .await?
        .map(|p| p.id)
        .ok_or_else(|| "No active profile found".to_string())?;
    analytics_service::stats(service.db(), &profile, &input).await
}
