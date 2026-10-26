ALTER TABLE souvenirs ADD COLUMN title TEXT CHECK(title IS NULL OR length(trim(title)) BETWEEN 1 AND 60);
