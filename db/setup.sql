
CREATE TABLE mods (
    id TEXT PRIMARY KEY,
    name TEXT,
    developer TEXT,
    download_url TEXT,
);

CREATE TABLE mods_access (
    mod_id TEXT,
    user_id INT,
    FOREIGN KEY(mod_id) REFERENCES mods(id),
    FOREIGN KEY(user_id) REFERENCES modders(user_id)
);

CREATE TABLE modders (
    user_id INT PRIMARY KEY,
    name TEXT,
    about_me TEXT
);
