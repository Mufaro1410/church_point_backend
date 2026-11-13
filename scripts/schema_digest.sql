-- Run in the Supabase SQL editor and save the single JSON cell it returns as
-- tests/fixtures/production_columns.json. The test `replica_matches_production_columns` then proves
-- the local replica still matches production (run it again whenever the real schema changes).
select json_object_agg(table_name, cols order by table_name) as production_columns
from (
  select table_name,
         string_agg(column_name || ' ' || udt_name
           || case when is_nullable = 'NO' then ' NOT NULL' else '' end
           || coalesce(' DEFAULT ' || column_default, ''), chr(10) order by ordinal_position) as cols
  from information_schema.columns
  where table_schema = 'public'
  group by table_name
) t;
