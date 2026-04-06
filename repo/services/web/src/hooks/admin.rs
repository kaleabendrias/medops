use contracts::UserSummaryDto;
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct AdminState {
    pub users: Signal<Vec<UserSummaryDto>>,
}

pub fn use_admin_state() -> AdminState {
    AdminState {
        users: use_signal(Vec::<UserSummaryDto>::new),
    }
}
