use contracts::{DishCategoryDto, DishDto, RankingRuleDto, RecommendationDto};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct DiningState {
    pub categories: Signal<Vec<DishCategoryDto>>,
    pub dishes: Signal<Vec<DishDto>>,
    pub ranking_rules: Signal<Vec<RankingRuleDto>>,
    pub recommendations: Signal<Vec<RecommendationDto>>,
    pub dish_category_id: Signal<String>,
    pub dish_name: Signal<String>,
    pub dish_description: Signal<String>,
    pub dish_price: Signal<String>,
    pub dish_photo_path: Signal<String>,
    pub dish_status_id: Signal<String>,
    pub dish_published: Signal<bool>,
    pub dish_sold_out: Signal<bool>,
    pub dish_option_id: Signal<String>,
    pub dish_option_group: Signal<String>,
    pub dish_option_value: Signal<String>,
    pub dish_option_delta: Signal<String>,
    pub dish_window_id: Signal<String>,
    pub dish_window_slot: Signal<String>,
    pub dish_window_start: Signal<String>,
    pub dish_window_end: Signal<String>,
    pub ranking_rule_key: Signal<String>,
    pub ranking_rule_weight: Signal<String>,
    pub ranking_rule_enabled: Signal<bool>,
}

pub fn use_dining_state() -> DiningState {
    DiningState {
        categories: use_signal(Vec::<DishCategoryDto>::new),
        dishes: use_signal(Vec::<DishDto>::new),
        ranking_rules: use_signal(Vec::<RankingRuleDto>::new),
        recommendations: use_signal(Vec::<RecommendationDto>::new),
        dish_category_id: use_signal(String::new),
        dish_name: use_signal(String::new),
        dish_description: use_signal(String::new),
        dish_price: use_signal(|| "0".to_string()),
        dish_photo_path: use_signal(|| "/var/lib/rocket-api/dishes/new.jpg".to_string()),
        dish_status_id: use_signal(String::new),
        dish_published: use_signal(|| true),
        dish_sold_out: use_signal(|| false),
        dish_option_id: use_signal(String::new),
        dish_option_group: use_signal(String::new),
        dish_option_value: use_signal(String::new),
        dish_option_delta: use_signal(|| "0".to_string()),
        dish_window_id: use_signal(String::new),
        dish_window_slot: use_signal(|| "Lunch".to_string()),
        dish_window_start: use_signal(|| "11:00".to_string()),
        dish_window_end: use_signal(|| "14:00".to_string()),
        ranking_rule_key: use_signal(String::new),
        ranking_rule_weight: use_signal(|| "0.5".to_string()),
        ranking_rule_enabled: use_signal(|| true),
    }
}
