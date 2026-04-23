use crate::database::*;
use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

#[tauri::command]
pub fn get_usage_logs(
    state: State<'_, AppState>,
    filter: UsageLogFilter,
) -> Result<PaginatedResult<UsageLog>, AppError> {
    state.db.get_usage_logs(&filter)
}

#[tauri::command]
pub fn get_dashboard_stats(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<DashboardStats, AppError> {
    let (start, end, _) = parse_filter(filter);
    state.db.get_dashboard_stats(start, end)
}

#[tauri::command]
pub fn get_model_consumption(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    let (start, end, granularity) = parse_filter(filter);
    state
        .db
        .get_model_consumption(start, end, granularity.as_deref())
}

#[tauri::command]
pub fn get_call_trend(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    let (start, end, granularity) = parse_filter(filter);
    state.db.get_call_trend(start, end, granularity.as_deref())
}

#[tauri::command]
pub fn get_model_distribution(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ModelRanking>, AppError> {
    let (start, end, _) = parse_filter(filter);
    state.db.get_model_distribution(start, end)
}

#[tauri::command]
pub fn get_model_ranking(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ModelRanking>, AppError> {
    let (start, end, _) = parse_filter(filter);
    state.db.get_model_ranking(start, end)
}

#[tauri::command]
pub fn get_user_ranking(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<UserRanking>, AppError> {
    let (start, end, _) = parse_filter(filter);
    state.db.get_user_ranking(start, end)
}

#[tauri::command]
pub fn get_user_trend(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    let (start, end, granularity) = parse_filter(filter);
    state.db.get_user_trend(start, end, granularity.as_deref())
}

#[derive(Deserialize)]
pub struct DashboardFilterParams {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub granularity: Option<String>,
}

fn parse_filter(
    filter: Option<DashboardFilterParams>,
) -> (Option<i64>, Option<i64>, Option<String>) {
    match filter {
        Some(f) => (f.start_time, f.end_time, f.granularity),
        None => (None, None, None),
    }
}
