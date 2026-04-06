-- Autonomous campaign closure via MySQL scheduled event.
-- Replaces opportunistic API-driven closure with a background event
-- that runs every minute, closing campaigns that are inactive (30+ min)
-- or past their success deadline.

-- event_scheduler=ON is set via MySQL server startup flag in docker-compose.yml

CREATE EVENT IF NOT EXISTS evt_close_inactive_campaigns
ON SCHEDULE EVERY 1 MINUTE
STARTS CURRENT_TIMESTAMP
DO
BEGIN
    -- Stage 1: Mark campaigns as Successful when qualifying orders meet threshold
    UPDATE group_campaigns gc
    JOIN (
        SELECT gc_inner.id AS campaign_id,
               COUNT(o.id) AS qualifying_orders
        FROM group_campaigns gc_inner
        LEFT JOIN campaign_members cm ON cm.campaign_id = gc_inner.id
        LEFT JOIN dishes d ON d.id = gc_inner.dish_id
        LEFT JOIN dining_menus dm ON dm.item_name = d.name
        LEFT JOIN dining_orders o
           ON o.menu_id = dm.id
          AND o.created_by = cm.user_id
          AND o.created_at >= gc_inner.created_at
          AND o.created_at <= gc_inner.success_deadline_at
          AND o.status IN ('Created', 'Billed')
        GROUP BY gc_inner.id
    ) qc ON qc.campaign_id = gc.id
    SET gc.status = 'Successful', gc.closed_at = COALESCE(gc.closed_at, NOW())
    WHERE gc.status = 'Open' AND qc.qualifying_orders >= gc.success_threshold;

    -- Stage 2: Close campaigns that are inactive (30+ min) or past deadline
    UPDATE group_campaigns
    SET status = 'Closed', closed_at = NOW()
    WHERE status = 'Open'
      AND (TIMESTAMPDIFF(MINUTE, last_activity_at, NOW()) >= 30
           OR NOW() > success_deadline_at);
END;
