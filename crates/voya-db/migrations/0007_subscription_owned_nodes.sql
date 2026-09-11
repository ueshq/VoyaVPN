-- Subscription provenance is the sole grouping authority for source-owned nodes.
-- Keep all nodes, subscriptions and manual folders; only obsolete memberships go.
DELETE FROM node_group_memberships
WHERE profile_id IN (SELECT index_id FROM profile_items WHERE subscription_id IS NOT NULL);
