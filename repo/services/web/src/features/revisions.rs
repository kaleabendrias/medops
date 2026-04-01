use crate::ui_logic::{parse_revision_deltas, RevisionDelta};
use contracts::RevisionTimelineDto;

pub fn deltas_for(revision: &RevisionTimelineDto) -> Vec<RevisionDelta> {
    parse_revision_deltas(&revision.field_deltas_json)
}

#[cfg(test)]
mod tests {
    use super::deltas_for;
    use contracts::RevisionTimelineDto;

    #[test]
    fn extracts_revision_delta_items() {
        let rev = RevisionTimelineDto {
            id: 1,
            entity_type: "demographics".to_string(),
            diff_before: "{}".to_string(),
            diff_after: "{}".to_string(),
            field_deltas_json:
                "[{\"field\":\"first_name\",\"before\":\"Old\",\"after\":\"New\",\"sensitive\":false}]"
                    .to_string(),
            reason_for_change: "fix".to_string(),
            actor_username: "admin".to_string(),
            created_at: "2026-01-01 00:00:00".to_string(),
        };
        let deltas = deltas_for(&rev);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].field, "first_name");
    }
}
