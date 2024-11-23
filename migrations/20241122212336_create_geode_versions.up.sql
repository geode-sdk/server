CREATE TABLE geode_versions (
	tag TEXT PRIMARY KEY NOT NULL,
	mac gd_version,
	win gd_version,
	android gd_version,
	commit_hash TEXT NOT NULL,
	prerelease BOOLEAN DEFAULT FALSE NOT NULL,
	created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- prefill some versions for the android updater
INSERT INTO geode_versions
	(tag, mac, win, android, created_at, commit_hash, prerelease) VALUES
	('4.0.1', '2.2074', '2.2074', '2.2074', '2024-11-20T21:39:54Z', 'ed97f3b0405a63f7edfa98df8d493a327e844111', FALSE),
	('4.0.0', '2.2074', '2.2074', '2.2074', '2024-11-19T14:40:29Z', 'cb8d7571ddcff608d3a4de8f59f1a969ec0aff39', FALSE),
	('4.0.0-beta.2', '2.2074', '2.2074', '2.2074', '2024-11-19T03:37:04Z', 'c0514b191583d6002dbf5c4f387471cff8fa535e', TRUE),
	('4.0.0-beta.1', '2.2074', '2.2074', '2.2074', '2024-11-15T20:15:17Z', '9fe3d133e93191d6225f2521b0714788293995c6', TRUE),
	('4.0.0-alpha.1', '2.2074', '2.2074', '2.2074', '2024-11-13T16:38:10Z', 'ebd4c920f5775287aea82fe758edec000108ff04', TRUE),
	('3.9.3', '2.206', '2.206', '2.206', '2024-11-22T19:05:51Z', 'e363a3e44c4a0af5b8980de4b06ed4c17e7b92ae', FALSE),
	('3.9.2', '2.206', '2.206', '2.206', '2024-11-14T22:01:58Z', '948e0d453dec9ea814974b17c68fde4006629611', FALSE),
	('3.9.1', '2.206', '2.206', '2.206', '2024-11-14T00:39:09Z', 'c4f6758ab42717816e121bcc656701fc7fd14395', FALSE),
	('3.9.0', '2.206', '2.206', '2.206', '2024-10-30T18:29:44Z', 'bd8387df1bc0bbba06a332c655e833121eddd9ff', FALSE),
	('2.0.0-beta.27', '2.200', '2.204', '2.205', '2024-05-26T14:37:03Z', '6510df7c8557668744044471ed2a0391759c3f7f', FALSE),
	('2.0.0-beta.4', '2.200', '2.204', '2.200', '2024-01-21T16:37:45Z', 'c2f626b93767ef3678b9df1daca14b89fec6c6f7', FALSE);

