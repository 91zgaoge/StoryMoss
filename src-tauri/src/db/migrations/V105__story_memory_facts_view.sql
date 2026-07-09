-- Unified read model over kg_entities + memory_items (non-destructive)
-- Physical SoT tables remain - this VIEW is a projection only
CREATE VIEW IF NOT EXISTS story_memory_facts AS
SELECT
  id,
  story_id,
  'kg_entity' AS record_kind,
  entity_type AS category,
  name AS subject,
  NULL AS field,
  COALESCE(json_extract(attributes, '$.description'), '') AS value,
  NULL AS source_chapter,
  COALESCE(confidence_score, 1.0) AS confidence,
  CASE WHEN is_archived = 1 THEN 'archived' ELSE 'active' END AS status,
  last_updated AS updated_at,
  id AS kg_entity_id,
  NULL AS memory_item_id
FROM kg_entities
UNION ALL
SELECT
  id,
  story_id,
  'memory_item' AS record_kind,
  category,
  subject,
  field,
  COALESCE(value, ''),
  source_chapter,
  confidence,
  status,
  updated_at,
  NULL AS kg_entity_id,
  id AS memory_item_id
FROM memory_items
