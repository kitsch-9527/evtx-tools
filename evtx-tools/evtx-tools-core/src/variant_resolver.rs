use crate::resolver::{norm_key, norm_provider, MessageDb};
use crate::{MessageRow, VariantKey};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

impl MessageDb {
    /// Batch-load variant rows for given (provider, event_id) cohorts (v6 precise).
    pub fn warmup_variants(&mut self, keys: &[VariantKey], lang: &str) -> crate::Result<()> {
        if keys.is_empty() { return Ok(()); }
        // R48：cohort 键规范化——事件侧 provider 大小写任意（'Disk' 命中 'disk'）。
        let mut cohorts: HashSet<(String, u32)> = HashSet::new();
        for k in keys { cohorts.insert((norm_provider(&k.provider_name), k.event_id)); }
        let cohorts: Vec<(String, u32)> = cohorts.into_iter().collect();
        let mut variant_rows = self.load_variant_rows_for_cohorts(lang, &cohorts)?;
        // R21（2026-09-29）：跨语言回落（MUI 回退链 zh-CN → en-US）。目标语言无行的
        // cohort 回落加载 en-US 行——wevtapi 在 zh 机器上对无 zh 资源的 provider 同样
        // 渲染 en（Setup 通道即此形态），此前 WHERE lang=?1 硬过滤使这类 cohort 在
        // zh 轮全空 → 占位文本假阳性（System-Restore 8300 族根因）。
        if lang != "en-US" {
            let loaded: HashSet<(String, u32)> = variant_rows
                .iter()
                .map(|(k, _)| (k.provider_name.clone(), k.event_id))
                .collect();
            let missing: Vec<(String, u32)> = cohorts
                .iter()
                .filter(|c| !loaded.contains(*c))
                .cloned()
                .collect();
            if !missing.is_empty() {
                let fallback_rows = self.load_variant_rows_for_cohorts("en-US", &missing)?;
                variant_rows.extend(fallback_rows);
            }
        }
        for (k, r) in variant_rows {
            let cohort_key = (k.provider_name.clone(), k.event_id);
            if self.variant_cache.insert(k.clone(), Arc::new(r)).is_none() {
                self.cohort_cache.entry(cohort_key).or_default().push(k);
            }
        }
        Ok(())
    }

    fn load_variant_rows_for_cohorts(
        &mut self,
        lang: &str,
        cohorts: &[(String, u32)],
    ) -> crate::Result<Vec<(VariantKey, MessageRow)>> {
        if cohorts.is_empty() { return Ok(Vec::new()); }
        let mut conditions = Vec::with_capacity(cohorts.len());
        let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(cohorts.len()*2+1);
        params.push(rusqlite::types::Value::Text(lang.to_string()));
        for (i, (prov, eid)) in cohorts.iter().enumerate() {
            let p_idx = i*2+2; let e_idx = i*2+3;
            // R48：COLLATE NOCASE——cohort 键已是规范形，DB 侧大小写任意。
            conditions.push(format!("(m.provider_name COLLATE NOCASE=?{} AND m.event_id=?{})", p_idx, e_idx));
            params.push(rusqlite::types::Value::Text(prov.clone()));
            params.push(rusqlite::types::Value::Integer(*eid as i64));
        }
        let query = format!(
            "SELECT m.provider_name, m.event_id, m.version, m.level, m.opcode, m.task, m.keywords, m.channel, m.message_identifier, m.message, m.parameters, m.level_name, m.keywords_name FROM provider_message_variants m WHERE m.lang=?1 AND ({})",
            conditions.join(" OR ")
        );
        let mut variant_rows: Vec<(VariantKey, MessageRow)> = Vec::new();
        {
            let mut stmt = self.conn.prepare(&query).map_err(|e| format!("prepare variant warmup: {e}"))?;
            let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
            let rows = stmt.query_map(refs.as_slice(), |row: &rusqlite::Row| {
                let pn: String = row.get(0)?;
                let eid: i64 = row.get(1)?;
                let ver: i64 = row.get(2)?;
                let lvl: i64 = row.get(3)?;
                let opc: i64 = row.get(4)?;
                let task: i64 = row.get(5)?;
                let kw: u64 = {
                    if let Ok(s) = row.get::<usize, String>(6) { s.parse::<u64>().unwrap_or(0) }
                    else if let Ok(v) = row.get::<usize, i64>(6) { v as u64 }
                    else if let Ok(v) = row.get::<usize, f64>(6) { v as u64 }
                    else { 0 }
                };
                let ch: i64 = row.get(7)?;
                let mid: i64 = row.get(8)?;
                let msg: String = row.get(9)?;
                let pj: Option<String> = row.get(10)?;
                let ln: Option<String> = row.get(11)?;
                let kn: Option<String> = row.get(12)?;
                let params = pj.and_then(|s: String| serde_json::from_str::<HashMap<String,String>>(&s).ok());
                // R48：装载即规范化——缓存键统一小写规范形。
                Ok((VariantKey{ provider_name: norm_provider(&pn), event_id: eid as u32, version: ver as u8, level: lvl as u8, opcode: opc as u8, task: task as u16, keywords: kw, channel: ch as u8, message_id: mid as u32 }, MessageRow{ template: msg, level: Some(lvl as u8), task: None, opcode: None, keywords: None, parameters: params, level_name: ln, keywords_name: kn, param_map_refs: HashMap::new(), param_bitmap_refs: HashSet::new(), param_types: HashMap::new()}))
            }).map_err(|e| format!("query variant warmup: {e}"))?;
            for row in rows { if let Ok(pair) = row { variant_rows.push(pair); } }
        }
        Ok(variant_rows)
    }
    pub fn get_variant(&self, key: &VariantKey) -> Option<Arc<MessageRow>> {
        // R48：入口规范化（幂等）——外部直调也安全。
        self.variant_cache.get(&norm_key(key)).cloned()
    }
    pub fn resolve_variant(&self, key: &VariantKey) -> Option<Arc<MessageRow>> {
        // R48：规范化一次，精确命中与 cohort 梯子共用规范键。
        let key = &norm_key(key);
        if let Some(r) = self.variant_cache.get(key).cloned() { return Some(r); }
        let mut best: Option<(&VariantKey, &Arc<MessageRow>, i32)> = None;
        // Cohort-scoped ladder: only this (provider, event_id)'s cached rows
        // can win, so scan them instead of the whole cache. Tie-break (R8)
        // is deterministic (score, then smaller message_id), so the scan
        // order over the cohort vec is irrelevant.
        let empty: Vec<VariantKey> = Vec::new();
        let cohort = self
            .cohort_cache
            .get(&(key.provider_name.clone(), key.event_id))
            .unwrap_or(&empty);
        for k in cohort {
            let v = match self.variant_cache.get(k) {
                Some(v) => v,
                None => continue,
            };
            let mut score: i32 = 0;
            if k.level != 0 { if k.level == key.level { score += 10; } else { score -= 10; } }
            if k.opcode != 0 { if k.opcode == key.opcode { score += 10; } else { score -= 10; } }
            if k.task != 0 { if k.task == key.task { score += 10; } else { score -= 10; } }
            if k.channel != 0 { if k.channel == key.channel { score += 10; } else { score -= 10; } }
            if k.keywords != 0 { if k.keywords == key.keywords { score += 10; } else { score -= 5; } }
            if k.version != 0 { if k.version == key.version { score += 10; } else { score -= 10; } } else { score += k.version as i32; }
            // R8（2026-09-04）：平分仲裁。事件维度为通配 0 / variant 全员不等分时（如 SPP16388
            // task=64 vs 192，事件 task=0），各候选同分，原 `score > best` 把胜负交给 HashMap
            // 迭代顺序——跨进程不确定，且生产实证选错（Authorized upgrade vs wevtapi 的
            // The current time is）。live 取证 wevtapi 选 message_identifier 最小者
            // （16388: 1073758212 < 3221241860；1016: 1073742840 < 2147484664），平分取小。
            let take = match &best {
                None => true,
                Some((bk, _, bs)) => score > *bs || (score == *bs && k.message_id < bk.message_id),
            };
            if take { best = Some((k, v, score)); }
        }
        if let Some((_, row, _)) = best { return Some(Arc::clone(row)); }
        // Single-file v7 (ADR 0003): no legacy single-template cache to fall back to.
        // Cohort miss means the (provider, event_id) is absent from the DB.
        None
    }
    /// R26（2026-09-30）：classic Qualifier 全键探针——在 (provider, event_id) cohort 内
    /// 只看 message_identifier == full_id 的候选（(Qualifiers<<16)|eid 精确身份，
    /// wevtapi 全键查找语义）。无命中返回 None，调用方回落 resolve_variant 梯子。
    pub fn resolve_variant_full_id(
        &self,
        key: &VariantKey,
        full_id: u32,
    ) -> Option<Arc<MessageRow>> {
        // R48：cohort 键规范化，与 resolve_variant 同规则。
        let key = &norm_key(key);
        let mut best: Option<(&VariantKey, &Arc<MessageRow>, i32)> = None;
        let empty: Vec<VariantKey> = Vec::new();
        let cohort = self
            .cohort_cache
            .get(&(key.provider_name.clone(), key.event_id))
            .unwrap_or(&empty);
        for k in cohort {
            if k.message_id != full_id {
                continue;
            }
            let v = match self.variant_cache.get(k) {
                Some(v) => v,
                None => continue,
            };
            let mut score: i32 = 0;
            if k.level != 0 { if k.level == key.level { score += 10; } else { score -= 10; } }
            if k.opcode != 0 { if k.opcode == key.opcode { score += 10; } else { score -= 10; } }
            if k.task != 0 { if k.task == key.task { score += 10; } else { score -= 10; } }
            if k.channel != 0 { if k.channel == key.channel { score += 10; } else { score -= 10; } }
            if k.keywords != 0 { if k.keywords == key.keywords { score += 10; } else { score -= 5; } }
            if k.version != 0 { if k.version == key.version { score += 10; } else { score -= 10; } } else { score += k.version as i32; }
            let take = match &best {
                None => true,
                Some((bk, _, bs)) => score > *bs || (score == *bs && k.message_id < bk.message_id),
            };
            if take { best = Some((k, v, score)); }
        }
        best.map(|(_, row, _)| Arc::clone(row))
    }
}
