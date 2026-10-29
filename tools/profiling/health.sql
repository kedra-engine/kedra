-- Run with the official Trace Processor under an external process deadline.
-- No row means no reported signal, NOT proof that all collection was lossless.
SELECT 'processor:' || name AS signal, CAST(value AS TEXT) AS value
FROM stats
WHERE value != 0 AND (
  severity IN ('error', 'data_loss') OR
  name GLOB '*overwrit*' OR name GLOB '*discard*' OR
  name GLOB '*packet_loss*' OR name GLOB '*incremental_state*'
)
UNION ALL
SELECT 'decoded_slices', CAST(COUNT(*) AS TEXT) FROM slice
UNION ALL
SELECT 'unfinished_scopes', CAST(COUNT(*) AS TEXT) FROM slice WHERE dur < 0
UNION ALL
SELECT 'recording_context_events', CAST(COUNT(*) AS TEXT)
FROM slice WHERE name = 'kedra.recording'
ORDER BY signal LIMIT 200;
