SELECT
    m.id,
    m.repository,
    m.latest_version,
    m.validated,
    mv.id as version_id,
    mv.name,
    mv.description,
    mv.version,
    mv.download_link,
    mv.hash,
    mv.geode_version,
    mv.windows,
    mv.android32,
    mv.android64,
    mv.mac,
    mv.ios
FROM mods m
INNER JOIN mod_versions mv ON m.id = mv.mod_id;