CREATE TABLE gd_version_aliases (
		version_name gd_version PRIMARY KEY NOT NULL,
		mac_arm_uuid uuid UNIQUE,
    mac_intel_uuid uuid UNIQUE,
    android_manifest_id INTEGER UNIQUE,
    windows_timestamp INTEGER UNIQUE,
		added_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- yet again, some default values, idk
INSERT INTO gd_version_aliases
	(version_name, mac_arm_uuid, mac_intel_uuid, android_manifest_id, windows_timestamp) VALUES
	-- not bothering with the mac uuids for most versions
	-- it's not like they'll ever be used by the updater anyways
	('2.200', null, null, 37, 1702921605),
	('2.204', null, null, null, 1705041028),
	('2.205', null, null, 38, null),
	('2.206', null, null, 39, 1717243515),
	('2.207', null, null, null, 1731098609),
	('2.2071', null, null, null, 1731117052),
	('2.2072', null, null, null, 1731130219),
	('2.2073', null, null, null, 1731156923),
	('2.2074', '27044C8B-76BD-303C-A035-5314AF1D9E6E', 'DB5CADC0-E533-3123-8A63-5A434FE391ED', 40, 1731376950);
