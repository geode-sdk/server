CREATE TABLE mods (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL DEFAULT 'My Mod',
    developer TEXT NOT NULL DEFAULT 'Developer',
    download_url TEXT
);

CREATE TABLE mods_access (
    mod_id TEXT NOT NULL,
    user_id INT NOT NULL,
    FOREIGN KEY(mod_id) REFERENCES mods(id),
    FOREIGN KEY(user_id) REFERENCES modders(user_id)
);

CREATE TABLE modders (
    user_id INT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL DEFAULT 'Developer',
    about_me TEXT
);
