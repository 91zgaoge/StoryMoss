-- Optional link from memory_items to kg_entities (non-destructive)
ALTER TABLE memory_items ADD COLUMN kg_entity_id TEXT REFERENCES kg_entities(id);
CREATE INDEX IF NOT EXISTS idx_memory_items_kg_entity ON memory_items(kg_entity_id);

-- Backfill by subject name match within the same story (entity / character_state only)
UPDATE memory_items
SET kg_entity_id = (
    SELECT e.id
    FROM kg_entities e
    WHERE e.story_id = memory_items.story_id
      AND e.name = memory_items.subject
      AND e.is_archived = 0
    LIMIT 1
)
WHERE kg_entity_id IS NULL
  AND subject IS NOT NULL
  AND category IN ('entity', 'character_state');

-- Recreate VIEW so memory_item rows expose the linked kg_entity_id
DROP VIEW IF EXISTS story_memory_facts;
CREATE VIEW story_memory_facts AS
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
  kg_entity_id,
  id AS memory_item_id
FROM memory_items
