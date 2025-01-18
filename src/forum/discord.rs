use serde_json::{json, to_string, Value};

use crate::types::models::mod_entity::Mod;
use crate::types::models::mod_gd_version::GDVersionEnum;
use crate::types::models::mod_version::ModVersion;
use crate::types::models::mod_version_status::ModVersionStatusEnum;

fn gd_to_string(gd: Option<GDVersionEnum>) -> String {
    gd.map(|x| to_string(&x).ok()).flatten().unwrap_or("N/A".to_string()).replace("\"", "")
}

fn map_or_none<T, F>(x: Option<Vec<T>>, f: F, sep: &str) -> String
    where
    F: Fn(T) -> String
{
    let ret = x.map(|x| x.into_iter().map(f).collect::<Vec<String>>().join(sep)).unwrap_or("None".to_string());
    if ret.is_empty() {
        "None".to_string()
    } else {
        ret
    }
}

fn mod_embed(m: &Mod, v: &ModVersion, base_url: &str) -> Value {
    json!({
        "title": if m.featured {
            format!("⭐️ {}", v.name)
        } else {
            v.name.clone()
        },
        "description": v.description,
        "url": format!("https://geode-sdk.org/mods/{}?version={}", m.id, v.version),
        "thumbnail": {
            "url": format!("{}/v1/mods/{}/logo", base_url, m.id)
        },
        "fields": [
            {
                "name": "ID",
                "value": m.id,
                "inline": true
            },
            {
                "name": "Version",
                "value": v.version,
                "inline": true
            },
            {
                "name": "Geode",
                "value": v.geode,
                "inline": true
            },
            {
                "name": "Early Load",
                "value": v.early_load,
                "inline": true
            },
            {
                "name": "API",
                "value": v.api,
                "inline": true
            },
            {
                "name": "Developers",
                "value": m.developers.clone().into_iter().map(|d| {
                    if d.is_owner {
                        format!("**[{}](https://geode-sdk.org/mods?developer={})**", d.display_name, d.username)
                    } else {
                        format!("[{}](https://geode-sdk.org/mods?developer={})", d.display_name, d.username)
                    }
                }).collect::<Vec<String>>().join(", "),
                "inline": true
            },
            {
                "name": "Geometry Dash",
                "value": format!("Windows: {}\nAndroid (32-bit): {}\nAndroid (64-bit): {}\nmacOS (Intel): {}\nmacOS (ARM): {}",
                    gd_to_string(v.gd.win), gd_to_string(v.gd.android32), gd_to_string(v.gd.android64),
                    gd_to_string(v.gd.mac_intel), gd_to_string(v.gd.mac_arm)),
                "inline": false
            },
            {
                "name": "Dependencies",
                "value": map_or_none(v.dependencies.clone(), |d| format!("`{} {} ({})`", d.mod_id, d.version,
                    to_string(&d.importance).unwrap_or("unknown".to_string()).replace("\"", "")), "\n"),
                "inline": false
            },
            {
                "name": "Incompatibilities",
                "value": map_or_none(v.incompatibilities.clone(), |i| format!("`{} {} ({})`", i.mod_id, i.version,
                    to_string(&i.importance).unwrap_or("unknown".to_string()).replace("\"", "")), "\n"),
                "inline": false
            },
            {
                "name": "Source",
                "value": m.links.clone().map(|l| l.source).flatten().unwrap_or(m.repository.clone().unwrap_or("N/A".to_string())),
                "inline": true
            },
            {
                "name": "Community",
                "value": m.links.clone().map(|l| l.community).flatten().unwrap_or("N/A".to_string()),
                "inline": true
            },
            {
                "name": "Homepage",
                "value": m.links.clone().map(|l| l.homepage).flatten().unwrap_or("N/A".to_string()),
                "inline": true
            },
            {
                "name": "Hash",
                "value": format!("`{}`", v.hash),
                "inline": true
            },
            {
                "name": "Download",
                "value": v.download_link,
                "inline": true
            },
            {
                "name": "Tags",
                "value": map_or_none(v.tags.clone(), |t| format!("`{}`", t), ", "),
                "inline": true
            }
        ]
    })
}

pub async fn get_threads(
    guild_id: u64, channel_id: u64,
    token: &str
) -> Vec<Value> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("https://discord.com/api/v10/guilds/{}/threads/active", guild_id))
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await;
    if res.is_err() {
        return vec![];
    }
    let res = res.unwrap();
    if !res.status().is_success() {
        return vec![];
    }
    let res = res.json::<Value>().await;
    if res.is_err() {
        return vec![];
    }
    let res = res.unwrap()["threads"].clone();
    if !res.is_array() {
        return vec![];
    }

    let vec1 = res.as_array().unwrap().clone().into_iter()
        .filter(|t| t["parent_id"].as_str().unwrap_or("").to_string().parse::<u64>().unwrap_or(0) == channel_id)
        .collect::<Vec<Value>>();

    let res2 = client
        .get(format!("https://discord.com/api/v10/channels/{}/threads/archived/public", channel_id))
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await;
    if res2.is_err() {
        return vec1;
    }
    let res2 = res2.unwrap();
    if !res2.status().is_success() {
        return vec1;
    }
    let res2 = res2.json::<Value>().await;
    if res2.is_err() {
        return vec1;
    }
    let res2 = res2.unwrap()["threads"].clone();
    if !res2.is_array() {
        return vec1;
    }

    let vec2 = res2.as_array().unwrap().clone();

    [vec1, vec2].concat().into_iter()
        .filter(|t| t["thread_metadata"]["locked"].is_boolean() && !t["thread_metadata"]["locked"].as_bool().unwrap())
        .collect::<Vec<Value>>()
}

pub async fn create_or_update_thread(
    threads: Option<Vec<Value>>,
    guild_id: u64, channel_id: u64,
    token: &str,
    m: &Mod,
    v: &ModVersion,
    admin: &str,
    base_url: &str
) {
    if guild_id == 0 || channel_id == 0 || token.is_empty() {
        log::error!("Discord configuration is not set up. Not creating forum threads.");
        return;
    }

    let thread_vec = if threads.is_some() {
        threads.unwrap()
    } else {
        get_threads(guild_id, channel_id, token).await
    };

    let thread = thread_vec.iter().find(|t| {
        t["name"].as_str().unwrap_or("").contains(format!("({})", m.id).as_str())
    });

    let client = reqwest::Client::new();
    if thread.is_none() {
        if v.status != ModVersionStatusEnum::Pending {
            return;
        }

        let _ = client
            .post(format!("https://discord.com/api/v10/channels/{}/threads", channel_id))
            .header("Authorization", format!("Bot {}", token))
            .json(&json!({
                "name": format!("🕓 {} ({}) {}", v.name, m.id, v.version),
                "message": {
                    "embeds": [mod_embed(m, v, base_url)],
                    "components": [
                        {
                            "type": 1,
                            "components": [
                                {
                                    "type": 2,
                                    "style": 3,
                                    "label": "Accept",
                                    "emoji": {
                                        "id": Value::Null,
                                        "name": "✅"
                                    },
                                    "custom_id": "index/admin/accept:forum"
                                },
                                {
                                    "type": 2,
                                    "style": 4,
                                    "label": "Reject",
                                    "emoji": {
                                        "id": Value::Null,
                                        "name": "❌"
                                    },
                                    "custom_id": "index-admin/reject:forum"
                                }
                            ]
                        }
                    ]
                }
            }))
            .send()
            .await;
        return;
    }

    let thread_id = thread.unwrap()["id"].as_str().unwrap_or("");
    if thread_id.is_empty() {
        return;
    }

    if thread.unwrap()["name"].as_str().unwrap_or("").ends_with(format!("{} ({}) {}", v.name, m.id, v.version).as_str()) {
        if v.status == ModVersionStatusEnum::Pending {
            return;
        }

        let _ = client
            .post(format!("https://discord.com/api/v10/channels/{}/messages", thread_id))
            .header("Authorization", format!("Bot {}", token))
            .json(&json!({
                "content": format!("{}{}{}", match v.status {
                    ModVersionStatusEnum::Accepted => "Accepted",
                    ModVersionStatusEnum::Rejected => "Rejected",
                    _ => "",
                }, if !admin.is_empty() {
                    format!(" by {}", admin)
                } else {
                    "".to_string()
                }, if v.info.is_some() && !v.info.clone().unwrap().is_empty() {
                    format!(": `{}`", v.info.clone().unwrap())
                } else {
                    "".to_string()
                }),
                "message_reference": {
                    "message_id": thread_id,
                    "fail_if_not_exists": false
                }
            }))
            .send()
            .await;

        let _ = client
            .patch(format!("https://discord.com/api/v10/channels/{}", thread_id))
            .header("Authorization", format!("Bot {}", token))
            .json(&json!({
                "name": match v.status {
                    ModVersionStatusEnum::Accepted => format!("✅ {} ({}) {}", v.name, m.id, v.version),
                    ModVersionStatusEnum::Rejected => format!("❌ {} ({}) {}", v.name, m.id, v.version),
                    _ => format!("🕓 {} ({}) {}", v.name, m.id, v.version),
                },
                "locked": true,
                "archived": true
            }))
            .send()
            .await;

        return;
    }

    let _ = client
        .patch(format!("https://discord.com/api/v10/channels/{}", thread_id))
        .header("Authorization", format!("Bot {}", token))
        .json(&json!({
            "name": format!("🕓 {} ({}) {}", v.name, m.id, v.version)
        }))
        .send()
        .await;

    let _ = client
        .patch(format!("https://discord.com/api/v10/channels/{}/messages/{}", thread_id, thread_id))
        .header("Authorization", format!("Bot {}", token))
        .json(&json!({
            "embeds": [mod_embed(m, v, base_url)]
        }))
        .send()
        .await;
}
