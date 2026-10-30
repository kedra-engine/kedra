-- Completed thread scopes only: logical operations are not CPU work.
-- Own elapsed time subtracts direct children, not OS scheduler residency.
WITH children AS (
  SELECT parent_id, SUM(dur) AS elapsed, MIN(dur) AS minimum
  FROM slice GROUP BY parent_id
), completed AS (
  SELECT s.category, s.name, s.dur,
    s.dur - COALESCE(c.elapsed, 0) AS own
  FROM slice s JOIN thread_track t ON t.id = s.track_id
  LEFT JOIN children c ON c.parent_id = s.id
  WHERE s.dur >= 0 AND COALESCE(c.minimum, 0) >= 0
    AND s.dur >= COALESCE(c.elapsed, 0)
    AND COALESCE(s.category, '') NOT IN ('Scheduling', 'Metadata')
)
SELECT category, name, COUNT(*) AS calls,
  ROUND(SUM(dur) / 1e6, 3) AS inclusive_elapsed_ms,
  ROUND(SUM(own) / 1e6, 3) AS own_elapsed_ms,
  ROUND(MAX(dur) / 1e6, 3) AS max_elapsed_ms
FROM completed GROUP BY category, name
ORDER BY own_elapsed_ms DESC LIMIT 100;
