use contracts::{OrderDto, OrderStatusRequest};

pub const ORDER_STATUSES: [&str; 4] = ["Created", "Billed", "Canceled", "Credited"];

pub fn transition_requires_reason(status: &str) -> bool {
    matches!(status, "Canceled" | "Credited")
}

pub fn order_status_request(
    order_id_input: &str,
    status: &str,
    reason_input: &str,
    orders: &[OrderDto],
) -> Result<(i64, OrderStatusRequest), String> {
    let order_id = order_id_input
        .trim()
        .parse::<i64>()
        .map_err(|_| "Order ID must be a valid number".to_string())?;
    if !ORDER_STATUSES.contains(&status) {
        return Err("Status must be one of Created, Billed, Canceled, or Credited".to_string());
    }
    let reason = reason_input.trim();
    if transition_requires_reason(status) && reason.is_empty() {
        return Err("A reason is required when canceling or crediting an order".to_string());
    }

    let expected_version = orders
        .iter()
        .find(|item| item.id == order_id)
        .map(|item| item.version);

    Ok((
        order_id,
        OrderStatusRequest {
            status: status.to_string(),
            reason: if reason.is_empty() {
                None
            } else {
                Some(reason.to_string())
            },
            expected_version,
        },
    ))
}

pub fn friendly_order_status_error(message: &str) -> String {
    if message.contains("http 409") {
        return "Order update conflict detected. Refresh orders and retry the transition.".to_string();
    }
    if message.contains("http 401") || message.contains("http 403") {
        return "You are not authorized to update this order. Re-authenticate or verify your role permissions.".to_string();
    }
    if message.contains("invalid status transition") {
        return "This status change is not allowed from the current order state.".to_string();
    }
    message.to_string()
}

#[cfg(test)]
mod tests {
    use super::{friendly_order_status_error, order_status_request, transition_requires_reason};

    fn sample_order(id: i64, version: i32) -> contracts::OrderDto {
        contracts::OrderDto {
            id,
            patient_id: 1,
            menu_id: 1,
            status: "Created".to_string(),
            notes: "note".to_string(),
            version,
        }
    }

    #[test]
    fn requires_reason_for_canceled() {
        let req = order_status_request("7", "Canceled", "", &[sample_order(7, 3)]);
        assert!(req.is_err());
    }

    #[test]
    fn uses_expected_version_when_order_is_loaded() {
        let (_order_id, req) = order_status_request(
            "7",
            "Credited",
            "customer refund",
            &[sample_order(7, 3)],
        )
        .expect("should build request");
        assert_eq!(req.expected_version, Some(3));
        assert_eq!(req.reason.as_deref(), Some("customer refund"));
    }

    #[test]
    fn allows_reasonless_billed_transition() {
        let result = order_status_request("7", "Billed", "", &[sample_order(7, 1)]);
        assert!(result.is_ok());
        assert!(!transition_requires_reason("Billed"));
    }

    #[test]
    fn maps_conflict_to_actionable_message() {
        let mapped = friendly_order_status_error("http 409: version conflict");
        assert!(mapped.to_ascii_lowercase().contains("conflict"));
        assert!(mapped.to_ascii_lowercase().contains("refresh"));
    }

    #[test]
    fn maps_authorization_failures_to_actionable_message() {
        let mapped = friendly_order_status_error("http 403: forbidden");
        assert!(mapped.to_ascii_lowercase().contains("not authorized"));
    }
}
