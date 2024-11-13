-- Add down migration script here

ALTER TABLE mod_tags DROP COLUMN display_name;

UPDATE mod_tags SET name = 'cheats'
WHERE name = 'cheat';
