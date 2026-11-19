//! Compact activity rollup. Payload/catalog expansion is never required for the heatmap.
use crate::{store::*, Result};
use rusqlite::named_params;
use serde_json::{json, Value};
use std::collections::BTreeMap;
fn date(day: i64) -> String {
    let clock = (day * 86400) as libc::time_t;
    let mut time = std::mem::MaybeUninit::<libc::tm>::uninit();
    unsafe {
        libc::gmtime_r(&clock, time.as_mut_ptr());
        let t = time.assume_init();
        format!(
            "{:04}-{:02}-{:02}",
            t.tm_year + 1900,
            t.tm_mon + 1,
            t.tm_mday
        )
    }
}
fn day(values: &mut BTreeMap<i64, Value>, n: i64) -> &mut Value {
    values.entry(n).or_insert_with(||json!({"day":date(n),"jobs":[],"job_count":0,"asset_count":0,"milliseconds":0.,"unknown_time_count":0,"live":false,"last_job":null}))
}
fn range(
    values: &mut BTreeMap<i64, Value>,
    id: &str,
    start: f64,
    end: Option<f64>,
    live: bool,
    clock: f64,
) {
    let Some(end) = end else {
        let v = day(values, ((start + 32400.) / 86400.).floor() as i64);
        if v["last_job"] != id {
            v["job_count"] = json!(v["job_count"].as_u64().unwrap_or(0) + 1);
            v["last_job"] = json!(id);
            if v["jobs"].as_array().unwrap().len() < 50 {
                v["jobs"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":id,"time":start}));
            }
        }
        v["unknown_time_count"] = json!(v["unknown_time_count"].as_u64().unwrap_or(0) + 1);
        return;
    };
    let mut cursor = start;
    loop {
        let n = ((cursor + 32400.) / 86400.).floor() as i64;
        let next = end.min((n + 1) as f64 * 86400. - 32400.);
        let v = day(values, n);
        if v["last_job"] != id {
            v["job_count"] = json!(v["job_count"].as_u64().unwrap_or(0) + 1);
            v["last_job"] = json!(id);
            if v["jobs"].as_array().unwrap().len() < 50 {
                v["jobs"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":id,"time":cursor}));
            }
        }
        v["milliseconds"] =
            json!(v["milliseconds"].as_f64().unwrap_or(0.) + (next - cursor).max(0.) * 1000.);
        v["live"] = json!(v["live"] == true || live && next >= clock);
        if next >= end {
            break;
        }
        cursor = next;
    }
}
impl Application {
    pub fn dashboard_activity(&self, view: &str) -> Result<Value> {
        let clock = now();
        let after = clock - 366. * 86400.;
        let mut days = BTreeMap::new();
        // Ordered ranges let us union overlapping execution intervals per logical Job without loading all Jobs.
        let system_job = crate::system_ownership::native_sql(
            "json_object('kind','job','identity',json_object('id',n.id))",
            "",
        );
        let source_system = crate::system_ownership::native_sql(
            "json_object('kind','job','identity',json_object('id',j.id))",
            "",
        );
        let sql=format!("{}, execution_ranges AS(
          SELECT s.job,s.attempt,s.started start,COALESCE(s.ended,CASE WHEN s.state='running' THEN :clock END) finish,
            s.ended IS NULL AND s.state='running' live
          FROM stage_runs s WHERE s.stage IN('prepare','run')
          UNION ALL
          SELECT a.job,a.id,CASE WHEN s.attempt IS NOT NULL THEN s.accepted ELSE a.started END,
            COALESCE(a.ended,CASE WHEN j.state IN('running','preparing') THEN :clock END),
            a.ended IS NULL AND j.state IN('running','preparing')
          FROM attempts a JOIN jobs j ON j.id=a.job LEFT JOIN rust_attempt_starts s ON s.attempt=a.id
          WHERE NOT EXISTS(SELECT 1 FROM stage_runs stage WHERE stage.job=a.job AND stage.attempt=a.id AND stage.stage IN('prepare','run'))
        ), raw AS(
          SELECT COALESCE((SELECT target FROM consolidation_roots WHERE source=j.id AND source_attempt IS r.attempt ORDER BY target LIMIT 1),u.job_id,v.job_id,j.id) id,
            r.start,r.finish,r.live
          FROM execution_ranges r JOIN jobs j ON j.id=r.job LEFT JOIN execution_memberships w ON w.job=j.id LEFT JOIN execution_units u ON u.id=j.id
          LEFT JOIN reviewed_work v ON v.id=j.id AND v.entry_type='unit' AND u.id IS NULL
          WHERE NOT COALESCE(({source_system}),0) AND (:view='all' OR COALESCE((SELECT json_extract(o.presentation,'$.scope') FROM research_overlays o WHERE o.kind='job' AND o.id=j.id),json_extract(j.spec,'$.presentation.scope'),'research')=:view)
          AND (:view!='research' OR COALESCE(json_extract(j.spec,'$.presentation.role'),'main')='main' OR w.root IS NOT NULL)
          UNION ALL
          SELECT COALESCE(v.job_id,h.id),json_extract(i.value,'$.start'),json_extract(i.value,'$.end'),0
          FROM history_records h LEFT JOIN reviewed_work v ON v.id=h.id AND v.entry_type='unit'
          JOIN json_each(h.body,'$.entity.execution_intervals') i
          WHERE (:view='all' OR COALESCE(json_extract(h.body,'$.entity.presentation.scope'),'research')=:view)
          AND (:view!='research' OR COALESCE(json_extract(h.body,'$.entity.presentation.role'),'main')='main')
          AND h.kind='job_view' AND h.source IN('queue','loom-ops') AND h.rowid=(SELECT max(x.rowid) FROM history_records x WHERE x.id=h.id AND x.kind=h.kind AND x.source IN('queue','loom-ops'))
          AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.id=h.id) AND NOT EXISTS(SELECT 1 FROM job_workflows j WHERE j.id=h.id)
        ), normalized AS(SELECT id,
          CASE WHEN typeof(start) IN('integer','real') THEN start/CASE WHEN start>100000000000 THEN 1000. ELSE 1. END ELSE CAST(strftime('%s',start) AS REAL) END start,
          CASE WHEN typeof(finish) IN('integer','real') THEN finish/CASE WHEN finish>100000000000 THEN 1000. ELSE 1. END ELSE CAST(strftime('%s',finish) AS REAL) END finish,live FROM raw)
        SELECT n.id,max(start,:after) start,min(finish,:clock) finish,live FROM normalized n WHERE NOT COALESCE(({system_job}),0) AND start IS NOT NULL AND start<=:clock AND (finish IS NULL OR finish>=:after) ORDER BY id,start",crate::query::CONSOLIDATION_CTE);
        let mut q = self.db.prepare(&sql)?;
        let records = q.query_map(
            named_params! {":clock":clock,":after":after,":view":view},
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            },
        )?;
        let mut current: Option<(String, f64, Option<f64>, bool)> = None;
        for record in records {
            let (id, start, end, live) = record?;
            if let Some((previous, a, b, l)) = &mut current {
                if let (Some(previous_end), Some(next_end)) = (*b, end) {
                    if previous == &id && start <= previous_end {
                        *b = Some(previous_end.max(next_end));
                        *l |= live;
                        continue;
                    }
                }
                range(&mut days, previous, *a, *b, *l, clock);
            }
            current = Some((id, start, end, live));
        }
        if let Some((id, start, end, live)) = current {
            range(&mut days, &id, start, end, live, clock);
        }
        let system_entity = crate::system_ownership::native_sql(
            "json_object('kind','entity','identity',json_object('id',e.id,'revision',e.revision))",
            "",
        );
        let sql=format!("WITH stamps AS(
          SELECT e.id,MIN(v.created) stamp FROM all_entity_revisions e JOIN events v ON v.job=e.job AND v.attempt=json_extract(e.manifest,'$.attempt') AND v.code='complete'
          WHERE e.kind IN('dataset','model','experiment') AND NOT COALESCE(({system_entity}),0)
          AND (:view='all' OR COALESCE((SELECT json_extract(o.presentation,'$.scope') FROM research_overlays o WHERE o.kind='entity' AND o.id=e.id),json_extract(e.manifest,'$.presentation.scope'),(SELECT json_extract(j.spec,'$.presentation.scope') FROM jobs j WHERE j.id=e.job),'research')=:view)
          AND (:view!='research' OR COALESCE(json_extract(e.manifest,'$.presentation.role'),(SELECT json_extract(j.spec,'$.presentation.role') FROM jobs j WHERE j.id=e.job),'main')='main') GROUP BY e.id
          UNION ALL
          SELECT h.id,COALESCE(json_extract(h.body,'$.original.properties.registered_at'),json_extract(h.body,'$.entity.properties.registered_at'),json_extract(h.body,'$.original.properties.created_at'),json_extract(h.body,'$.entity.properties.created_at'))
          FROM history_records h WHERE (:view='all' OR COALESCE(json_extract(h.body,'$.original.presentation.scope'),json_extract(h.body,'$.entity.presentation.scope'),'research')=:view) AND h.source='catalog' AND h.kind IN('dataset','model','run','experiment')
          AND COALESCE(json_extract(h.body,'$.original.contribution.eligible'),json_extract(h.body,'$.entity.contribution.eligible'),0)=1
          AND h.rowid=(SELECT max(x.rowid) FROM history_records x WHERE x.source=h.source AND x.id=h.id)
          AND NOT EXISTS(SELECT 1 FROM all_entity_revisions e WHERE e.id=h.id)
        ), normalized AS(SELECT id,CASE WHEN typeof(stamp) IN('integer','real') THEN stamp/CASE WHEN stamp>100000000000 THEN 1000. ELSE 1. END ELSE CAST(strftime('%s',stamp) AS REAL) END stamp FROM stamps)
        SELECT CAST((stamp+32400.)/86400. AS INTEGER) day,count(DISTINCT id) count FROM normalized WHERE stamp BETWEEN :after AND :clock GROUP BY day");
        let mut q = self.db.prepare(&sql)?;
        for r in q.query_map(
            named_params! {":after":after,":clock":clock,":view":view},
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )? {
            let (n, count) = r?;
            day(&mut days, n)["asset_count"] = json!(count);
        }
        for v in days.values_mut() {
            v.as_object_mut().unwrap().remove("last_job");
        }
        Ok(
            json!({"schema":"loom-activity/v1","days":days.values().collect::<Vec<_>>(),"through":clock,"timezone":"Asia/Seoul","window_days":366,"detail_limit":50,"job_count_basis":"distinct Job or standalone Unit per KST day; attached Units and repeated Attempts roll up to their Job or Unit","basis":"actual execution intervals and recorded eligible asset registration; unknown time stays unknown"}),
        )
    }
}
