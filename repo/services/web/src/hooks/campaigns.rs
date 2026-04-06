use contracts::CampaignDto;
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct CampaignsState {
    pub campaigns: Signal<Vec<CampaignDto>>,
    pub campaign_title: Signal<String>,
    pub campaign_dish_id: Signal<String>,
    pub campaign_threshold: Signal<String>,
    pub campaign_deadline: Signal<String>,
    pub campaign_join_id: Signal<String>,
}

pub fn use_campaigns_state() -> CampaignsState {
    CampaignsState {
        campaigns: use_signal(Vec::<CampaignDto>::new),
        campaign_title: use_signal(String::new),
        campaign_dish_id: use_signal(|| "1".to_string()),
        campaign_threshold: use_signal(|| "5".to_string()),
        campaign_deadline: use_signal(|| "2099-01-01 10:30:00".to_string()),
        campaign_join_id: use_signal(String::new),
    }
}
