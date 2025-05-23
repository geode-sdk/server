CREATE TABLE geode_versions (
	tag TEXT PRIMARY KEY NOT NULL,
	mac gd_version,
	win gd_version,
	android gd_version,
	ios gd_version,
	commit_hash TEXT NOT NULL,
	prerelease BOOLEAN DEFAULT FALSE NOT NULL,
	created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- prefill some versions for the android updater
INSERT INTO geode_versions
	(tag, mac, win, android, ios, created_at, commit_hash, prerelease) VALUES
	('4.3.1', '2.2074', '2.2074', '2.2074', NULL, '2025-03-12T22:16:41Z', '25131977310c6325566e771add660c279ddd26fd', FALSE),
	('4.3.0', '2.2074', '2.2074', '2.2074', NULL, '2025-03-12T17:17:29Z', '75bf0dfef335f5a0ce5b2c46700875e52e801350', FALSE),
	('4.2.0', '2.2074', '2.2074', '2.2074', NULL, '2025-01-18T01:40:23Z', 'f540c39d251babf6981f1a197da9accee777b1e8', FALSE),
	('4.1.2', '2.2074', '2.2074', '2.2074', NULL, '2024-12-29T03:37:32Z', 'd9d8a1281bae20b51e4324825f22262499f40049', FALSE),
	('4.1.1', '2.2074', '2.2074', '2.2074', NULL, '2024-12-20T07:36:05Z', 'e558bd3d12b9527cccccfabc2b35444a538bf5f1', FALSE),
	('4.1.0', '2.2074', '2.2074', '2.2074', NULL, '2024-12-12T21:23:18Z', '695f39f6ae4d10a88929b4bdc336f980db9060b6', FALSE),
	('4.0.1', '2.2074', '2.2074', '2.2074', NULL, '2024-11-20T21:39:54Z', 'ed97f3b0405a63f7edfa98df8d493a327e844111', FALSE),
	('4.0.0', '2.2074', '2.2074', '2.2074', NULL, '2024-11-19T14:40:29Z', 'cb8d7571ddcff608d3a4de8f59f1a969ec0aff39', FALSE),
	('4.0.0-beta.2', '2.2074', '2.2074', '2.2074', NULL, '2024-11-19T03:37:04Z', 'c0514b191583d6002dbf5c4f387471cff8fa535e', TRUE),
	('4.0.0-beta.1', '2.2074', '2.2074', '2.2074', NULL, '2024-11-15T20:15:17Z', '9fe3d133e93191d6225f2521b0714788293995c6', TRUE),
	('4.0.0-alpha.1', '2.2074', '2.2074', '2.2074', NULL, '2024-11-13T16:38:10Z', 'ebd4c920f5775287aea82fe758edec000108ff04', TRUE),
	('3.9.3', '2.206', '2.206', '2.206', NULL, '2024-11-22T19:05:51Z', 'e363a3e44c4a0af5b8980de4b06ed4c17e7b92ae', FALSE),
	('3.9.2', '2.206', '2.206', '2.206', NULL, '2024-11-14T22:01:58Z', '948e0d453dec9ea814974b17c68fde4006629611', FALSE),
	('3.9.1', '2.206', '2.206', '2.206', NULL, '2024-11-14T00:39:09Z', 'c4f6758ab42717816e121bcc656701fc7fd14395', FALSE),
	('3.9.0', '2.206', '2.206', '2.206', NULL, '2024-10-30T18:29:44Z', 'bd8387df1bc0bbba06a332c655e833121eddd9ff', FALSE),
	('2.0.0-beta.27', '2.200', '2.204', '2.205', NULL, '2024-05-26T14:37:03Z', '6510df7c8557668744044471ed2a0391759c3f7f', FALSE),
	('2.0.0-beta.4', '2.200', '2.204', '2.200', NULL, '2024-01-21T16:37:45Z', 'c2f626b93767ef3678b9df1daca14b89fec6c6f7', FALSE);

CREATE INDEX idx_geode_versions_created_at ON geode_versions(created_at);
