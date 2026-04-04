use dioxus::prelude::*;

use contracts::{FunnelMetricsDto, RecommendationKpiDto, RetentionMetricsDto};

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn AnalyticsPage(
    session: Signal<Option<SessionContext>>,
    mut funnel: Signal<Vec<FunnelMetricsDto>>,
    mut retention: Signal<Vec<RetentionMetricsDto>>,
    mut recommendation_kpi: Signal<Option<RecommendationKpiDto>>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Analytics" }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                    spawn(async move {
                        if let Ok(items) = api::funnel_metrics(&token).await { funnel.set(items); }
                        if let Ok(items) = api::retention_metrics(&token).await { retention.set(items); }
                        if let Ok(item) = api::recommendation_kpi(&token).await { recommendation_kpi.set(Some(item)); }
                    });
                },
                "Refresh Analytics"
            }
            div { class: "cards", for f in funnel() { article { class: "card", p { "{f.step}: {f.users} users" } } } }
            div { class: "cards", for r in retention() { article { class: "card", p { "{r.cohort}: {r.retained_users} retained" } } } }
            if let Some(kpi) = recommendation_kpi() {
                article { class: "card", p { "Recommendation CTR: {kpi.ctr}" } p { "Recommendation conversion: {kpi.conversion}" } }
            }
        }
    }
}
