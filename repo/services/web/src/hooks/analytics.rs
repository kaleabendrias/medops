use contracts::{FunnelMetricsDto, RecommendationKpiDto, RetentionMetricsDto};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct AnalyticsState {
    pub funnel: Signal<Vec<FunnelMetricsDto>>,
    pub retention: Signal<Vec<RetentionMetricsDto>>,
    pub recommendation_kpi: Signal<Option<RecommendationKpiDto>>,
}

pub fn use_analytics_state() -> AnalyticsState {
    AnalyticsState {
        funnel: use_signal(Vec::<FunnelMetricsDto>::new),
        retention: use_signal(Vec::<RetentionMetricsDto>::new),
        recommendation_kpi: use_signal(|| None::<RecommendationKpiDto>),
    }
}
