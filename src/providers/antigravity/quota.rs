use crate::model::LimitWindow;
use crate::providers::http::{token, window};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A valid, even empty, summary is authoritative; absent fields are unknown.
pub fn summary(body: &Value) -> Option<Vec<LimitWindow>> {
    let groups = body
        .pointer("/response/groups")
        .or_else(|| body.get("groups"))?
        .as_array()?;
    let mut buckets = BTreeMap::new();
    for bucket in groups
        .iter()
        .filter_map(|group| group["buckets"].as_array())
        .flatten()
    {
        let Some(id) = bucket["bucketId"].as_str() else {
            continue;
        };
        let Some(fraction) = bucket["remainingFraction"]
            .as_f64()
            .filter(|f| f.is_finite())
        else {
            continue;
        };
        buckets
            .entry(id)
            .or_insert((fraction, &bucket["resetTime"]));
    }
    let sep = crate::analytics::separator();
    Some(
        [
            ("gemini-5h", format!("Gemini{sep}5-hour"), 300),
            ("gemini-weekly", format!("Gemini{sep}weekly"), 10080),
            ("3p-5h", format!("Claude / GPT{sep}5-hour"), 300),
            ("3p-weekly", format!("Claude / GPT{sep}weekly"), 10080),
        ]
        .into_iter()
        .filter_map(|(id, label, mins)| {
            let (fraction, reset) = buckets.get(id)?;
            let mut limit = window(
                &label,
                &json!((1. - fraction.clamp(0., 1.)) * 100.),
                reset,
                Some(mins),
            )?;
            if mins == 300 && *fraction >= 1. {
                limit.resets_at = None;
            }
            Some(limit)
        })
        .collect(),
    )
}

pub fn legacy(body: &Value) -> Vec<LimitWindow> {
    let mut configs = vec![];
    if let Some(models) = body["models"].as_object() {
        for (id, model) in models {
            if model["isInternal"] == true {
                continue;
            }
            configs.push((id.clone(), model));
        }
    }
    if let Some(models) = body
        .pointer("/userStatus/cascadeModelConfigData/clientModelConfigs")
        .or_else(|| body.get("clientModelConfigs"))
        .and_then(Value::as_array)
    {
        for model in models {
            configs.push((
                model
                    .pointer("/modelOrAlias/model")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                model,
            ));
        }
    }
    if let Some(models) = body["buckets"].as_array() {
        for model in models {
            configs.push((model["modelId"].as_str().unwrap_or("").into(), model));
        }
    }
    let mut pools: BTreeMap<bool, LimitWindow> = BTreeMap::new();
    for (id, config) in configs {
        if id.starts_with("MODEL_PLACEHOLDER")
            || matches!(
                id.as_str(),
                "MODEL_CHAT_20706"
                    | "MODEL_CHAT_23310"
                    | "MODEL_GOOGLE_GEMINI_2_5_FLASH"
                    | "MODEL_GOOGLE_GEMINI_2_5_FLASH_THINKING"
                    | "MODEL_GOOGLE_GEMINI_2_5_FLASH_LITE"
                    | "MODEL_GOOGLE_GEMINI_2_5_PRO"
            )
        {
            continue;
        }
        let quota = config.get("quotaInfo").unwrap_or(config);
        let Some(fraction) = quota["remainingFraction"]
            .as_f64()
            .filter(|f| f.is_finite())
        else {
            continue;
        };
        let name = config
            .get("label")
            .or_else(|| config.get("displayName"))
            .and_then(Value::as_str)
            .unwrap_or(&id);
        let gemini = format!("{id} {name}").to_lowercase().contains("gemini");
        if !gemini
            && !format!("{id} {name}").to_lowercase().contains("claude")
            && !format!("{id} {name}").to_lowercase().contains("gpt")
        {
            continue;
        }
        let sep = crate::analytics::separator();
        let label = if gemini {
            format!("Gemini{sep}5-hour")
        } else {
            format!("Claude / GPT{sep}5-hour")
        };
        let mut limit = window(
            &label,
            &json!((1. - fraction.clamp(0., 1.)) * 100.),
            &quota["resetTime"],
            Some(300),
        )
        .unwrap();
        if fraction >= 1. {
            limit.resets_at = None;
        }
        if pools
            .get(&gemini)
            .is_none_or(|old| old.used_percent < limit.used_percent)
        {
            pools.insert(gemini, limit);
        }
    }
    [true, false]
        .into_iter()
        .filter_map(|pool| pools.remove(&pool))
        .collect()
}
pub(super) fn plan(body: &Value) -> Option<String> {
    [
        "/userStatus/userTier/name",
        "/userStatus/planStatus/planInfo/planName",
        "/paidTier/name",
        "/currentTier/name",
    ]
    .into_iter()
    .find_map(|path| token(body, path))
}
