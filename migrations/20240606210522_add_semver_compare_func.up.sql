-- Add up migration script here

-- Taken from https://github.com/cbandy/semver

CREATE OR REPLACE FUNCTION semver_compare(text, text)
RETURNS integer AS $$
    SELECT CASE
        WHEN left_version IS NULL OR right_version IS NULL THEN NULL
        WHEN array_length(left_version, 1) < 3 OR array_length(right_version, 1) < 3 THEN NULL
        WHEN left_version < right_version THEN -1
        WHEN left_version > right_version THEN 1
        ELSE CASE
            WHEN left_prerelease IS NOT NULL AND right_prerelease IS NULL THEN -1
            WHEN left_prerelease IS NULL AND right_prerelease IS NOT NULL THEN 1
            WHEN left_prerelease < right_prerelease THEN -1
            WHEN left_prerelease > right_prerelease THEN 1
            ELSE 0
        END
    END
    FROM (
        SELECT
            string_to_array(left_portions[1], '.')::integer[],
            string_to_array(right_portions[1], '.')::integer[],

            (SELECT array_agg(CASE WHEN v ~ '^\d+$' THEN ROW(''::text, v::integer) ELSE ROW(v) END) FROM unnest(string_to_array(lower(left_portions[2]), '.')) x (v)),
            (SELECT array_agg(CASE WHEN v ~ '^\d+$' THEN ROW(''::text, v::integer) ELSE ROW(v) END) FROM unnest(string_to_array(lower(right_portions[2]), '.')) x (v))
        FROM (VALUES (
            (SELECT * FROM regexp_matches($1, '^([[:digit:].]+)(?:-([[:alnum:].-]+))?(?:\+[[:alnum:].-]+)?$')),
            (SELECT * FROM regexp_matches($2, '^([[:digit:].]+)(?:-([[:alnum:].-]+))?(?:\+[[:alnum:].-]+)?$'))
        )) x (left_portions, right_portions)
    ) x (left_version, right_version, left_prerelease, right_prerelease)
$$ LANGUAGE SQL IMMUTABLE STRICT;