-- Bible reading plans now repeat every year. Progress used to be saved against the day number (1..365),
-- which would carry into the next year. It is now saved against the date of the reading (YYYYMMDD),
-- so each year starts fresh and past years stay as history. Day-number keys are converted using the
-- plan's own dates (the year in those dates is the year they were read in).
update public.bible_reading_progress p
   set progress_data = (
        select coalesce(jsonb_object_agg(coalesce(m.new_key, e.key), e.value), '{}'::jsonb)
          from jsonb_each(p.progress_data) e
          left join lateral (
                select replace(r ->> 'date', '-', '') as new_key
                  from public.bible_reading_plans pl, jsonb_array_elements(pl.readings) r
                 where pl.id = p.plan_id and (r ->> 'day') = e.key and length(e.key) <= 3
                 limit 1) m on true)
 where exists (select 1 from jsonb_object_keys(p.progress_data) k where length(k) <= 3);
