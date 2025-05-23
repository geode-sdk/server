CREATE TABLE gd_version_aliases (
	version_name gd_version PRIMARY KEY NOT NULL,
	mac_arm_uuid uuid UNIQUE,
	mac_intel_uuid uuid UNIQUE,
	android_manifest_id INTEGER UNIQUE,
	windows_timestamp INTEGER UNIQUE,
	ios_bundle_version TEXT UNIQUE,
	added_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);

INSERT INTO gd_version_aliases
	(version_name, mac_arm_uuid, mac_intel_uuid, android_manifest_id, windows_timestamp, ios_bundle_version) VALUES
	-- thanks to @hiimjasmine00 for bothering with the mac uuids
	('2.200', null, '29549F90-F083-35A8-B917-9962262FE112', 37, 1702921605, '2.2.0'),
	('2.204', null, null, null, 1705041028, null),
	('2.205', null, null, 38, null, '2.2.0.5'),
	('2.206', '620B0C9B-8F75-3043-BD34-3BB9DD201C3A', 'AE6DFCCC-153A-32AB-BFD5-6F2478BC41B6', 39, 1717243515, '2.2.0.6.0'),
	('2.207', '48C25B63-0D7C-3F67-B831-DF935524C043', 'D497E431-5C3F-3EB4-9DF7-115B861578EE', null, 1731098609, null),
	('2.2071', '4933391F-D6C1-3188-99E8-23D64C674B64', '08E24832-EC11-3637-910E-7CB6C0EF8EC0', null, 1731117052, null),
	('2.2072', '9C1D62A7-7C2F-3514-AEFB-D1AB7BBD48FF', 'E53731FD-D1B6-33D2-BFA4-3B5D8D55279F', null, 1731130219, null),
	('2.2073', '0B1FCFE4-79E8-3246-8ECB-500FDBDCFD9A', '1F4AFF98-DB51-382D-9BB2-59C911B88EB2', null, 1731156923, null),
	('2.2074', '27044C8B-76BD-303C-A035-5314AF1D9E6E', 'DB5CADC0-E533-3123-8A63-5A434FE391ED', 40, 1731376950, '2.2.0.7.0');

CREATE INDEX idx_gd_version_aliases_added_at ON gd_version_aliases(added_at);
