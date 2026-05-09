-- Seed initial badges
INSERT INTO badges (name, description, criteria_type, criteria_value) VALUES
    ('First Sip', 'Rate your very first beer', 'total_ratings', 1),
    ('Explorer', 'Rate 10 different beers', 'total_ratings', 10),
    ('Connoisseur', 'Rate 50 different beers', 'total_ratings', 50),
    ('Style Hunter', 'Rate beers in 5 different styles', 'unique_styles', 5),
    ('Loyal Patron', 'Rate 5 beers from the same brewery', 'same_brewery', 5)
ON CONFLICT (name) DO NOTHING;

-- Seed initial encryption key version
INSERT INTO encryption_keys (key_version, active) VALUES (1, true)
ON CONFLICT (key_version) DO NOTHING;
