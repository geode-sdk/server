CREATE TABLE geode_versions (
		tag TEXT PRIMARY KEY NOT NULL,
		mac gd_version,
		win gd_version,
		android gd_version,
		prerelease BOOLEAN DEFAULT FALSE NOT NULL,
		created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- prefill some versions for the android updater
INSERT INTO geode_versions
	(tag, mac, win, android, created_at, prerelease) VALUES
	('v4.0.1', '2.2074', '2.2074', '2.2074', '2024-11-20T21:39:54Z', FALSE),
	('v4.0.0', '2.2074', '2.2074', '2.2074', '2024-11-19T14:40:29Z', FALSE),
	('v4.0.0-beta.2', '2.2074', '2.2074', '2.2074', '2024-11-19T03:37:04Z', TRUE),
	('v4.0.0-beta.1', '2.2074', '2.2074', '2.2074', '2024-11-15T20:15:17Z', TRUE),
	('v4.0.0-alpha.1', '2.2074', '2.2074', '2.2074', '2024-11-13T16:38:10Z', TRUE),
	('v3.9.3', '2.206', '2.206', '2.206', '2024-11-22T19:05:51Z', FALSE),
	('v3.9.2', '2.206', '2.206', '2.206', '2024-11-14T22:01:58Z', FALSE),
	('v3.9.1', '2.206', '2.206', '2.206', '2024-11-14T00:39:09Z', FALSE),
	('v3.9.0', '2.206', '2.206', '2.206', '2024-10-30T18:29:44Z', FALSE),
	('v2.0.0-beta.27', '2.200', '2.204', '2.205', '2024-05-26T14:37:03Z', FALSE),
	('v2.0.0-beta.4', '2.200', '2.204', '2.200', '2024-01-21T16:37:45Z', FALSE);

