ALTER TABLE settings ADD COLUMN ui_language TEXT NOT NULL DEFAULT 'auto' CHECK (ui_language IN ('auto', 'pl', 'en'));
