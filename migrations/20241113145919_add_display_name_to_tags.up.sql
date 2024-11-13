-- Add up migration script here

ALTER TABLE mod_tags ADD COLUMN display_name TEXT;

UPDATE mod_tags SET display_name = 'Universal'
WHERE name = 'universal';
UPDATE mod_tags SET display_name = 'Gameplay'
WHERE name = 'gameplay';
UPDATE mod_tags SET display_name = 'Editor'
WHERE name = 'editor';
UPDATE mod_tags SET display_name = 'Offline'
WHERE name = 'offline';
UPDATE mod_tags SET display_name = 'Online'
WHERE name = 'online';
UPDATE mod_tags SET display_name = 'Enhancement'
WHERE name = 'enhancement';
UPDATE mod_tags SET display_name = 'Music'
WHERE name = 'music';
UPDATE mod_tags SET display_name = 'Interface'
WHERE name = 'interface';
UPDATE mod_tags SET display_name = 'Bugfix'
WHERE name = 'bugfix';
UPDATE mod_tags SET display_name = 'Utility'
WHERE name = 'utility';
UPDATE mod_tags SET display_name = 'Performance'
WHERE name = 'performance';
UPDATE mod_tags SET display_name = 'Customization'
WHERE name = 'customization';
UPDATE mod_tags SET display_name = 'Content'
WHERE name = 'content';
UPDATE mod_tags SET display_name = 'Developer'
WHERE name = 'developer';
UPDATE mod_tags SET display_name = 'Cheat', name = 'cheat'
WHERE name = 'cheats';
UPDATE mod_tags SET display_name = 'Paid'
WHERE name = 'paid';
UPDATE mod_tags SET display_name = 'Joke'
WHERE name = 'joke';
UPDATE mod_tags SET display_name = 'Modtober 2024'
WHERE name = 'modtober24';
