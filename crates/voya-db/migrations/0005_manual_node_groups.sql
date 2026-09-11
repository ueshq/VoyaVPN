-- Retired executable profiles are removed before the new DTOs decode rows.
-- Keep routes that can still resolve to an ordinary node with the same name.
CREATE TEMP TABLE retired_node_names AS
SELECT DISTINCT remarks FROM profile_items
WHERE config_type IN ('policyGroup', 'proxyChain', 'custom');

DELETE FROM profile_items WHERE config_type IN ('policyGroup', 'proxyChain', 'custom');

UPDATE routing_items
SET rule_set = (
    SELECT json_group_array(json(value)) FROM json_each(routing_items.rule_set)
    WHERE COALESCE(json_extract(value, '$.outboundTag'), '') NOT IN (
        SELECT remarks FROM retired_node_names
        WHERE remarks NOT IN ('proxy', 'direct', 'block')
          AND NOT EXISTS (SELECT 1 FROM profile_items WHERE profile_items.remarks = retired_node_names.remarks)
    )
)
WHERE json_valid(rule_set) AND json_type(rule_set) = 'array';
DROP TABLE retired_node_names;

UPDATE app_settings SET payload = json_remove(payload, '$.behavior.autoCreateSubscriptionGroup', '$.proxy.nodeSorting', '$.speedTest.proxyDelayConcurrency', '$.speedTest.mixedConcurrency');
ALTER TABLE subscriptions DROP COLUMN pre_socks_port;

CREATE TABLE node_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) > 0),
    sort INTEGER NOT NULL
);
CREATE INDEX idx_node_groups_sort ON node_groups (sort, id);

CREATE TABLE node_group_memberships (
    profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profile_items(index_id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES node_groups(id) ON DELETE CASCADE
);
CREATE INDEX idx_node_group_memberships_group ON node_group_memberships (group_id);
