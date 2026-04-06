use dioxus::prelude::*;

use crate::ui_logic::{QueuedAttachment, UploadState};

#[derive(Clone, Copy)]
pub struct UploadsState {
    pub upload_progress: Signal<u8>,
    pub upload_state: Signal<String>,
    pub upload_state_kind: Signal<UploadState>,
    pub attachment_queue: Signal<Vec<QueuedAttachment>>,
}

pub fn use_uploads_state() -> UploadsState {
    UploadsState {
        upload_progress: use_signal(|| 0u8),
        upload_state: use_signal(String::new),
        upload_state_kind: use_signal(|| UploadState::Idle),
        attachment_queue: use_signal(Vec::<QueuedAttachment>::new),
    }
}

impl UploadsState {
    pub fn reset(&mut self) {
        self.attachment_queue.set(Vec::new());
        self.upload_state_kind.set(UploadState::Idle);
        self.upload_progress.set(0);
    }
}
