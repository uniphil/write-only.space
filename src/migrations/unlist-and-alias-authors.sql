ALTER TABLE author
    DROP COLUMN username,
    ADD COLUMN alias text,
    ADD COLUMN key uuid NOT NULL DEFAULT uuid_generate_v4();
