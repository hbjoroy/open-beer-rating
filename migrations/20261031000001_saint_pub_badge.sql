-- Existing tasting history is evaluated lazily when users view their badges.
-- No history, catalog entries or previously awarded badges are changed.
INSERT INTO badges (name, description, criteria_type, criteria_value)
VALUES ('Holy Hop', 'Check in a beer at a pub with an explicit saint name, such as St. Patrick or Sankt Olav.', 'saint_pub', 1)
ON CONFLICT (name) DO NOTHING;
