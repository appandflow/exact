//! What an update takes from the binary instead of the origin: an embedded
//! asset whose bytes are what the head declares, and nothing else.
//!
//! @ref LLP 1026 D11 (assets by digest, embedded assets included)

use crate::support::*;
use exact_update::{sha256_hex, Check, Client, EmbeddedAsset, Outcome};

const COMPAT: &str = r#"{"id":"abc","inputs":{"app":"com.exact.t","keys":null,"trust":"development","store":{"L":"A"}},"delivery":{"activate":"next-launch","channel":"prod","origin":"https://o.example"}}"#;
const CLIENT_ORIGIN: &str = "https://o.example";
const BASE: &str = "https://o.example/.exact/prod/abc";

/// A head at `seq` naming `plan` and `assets` (name, bytes), and every file
/// the origin serves for it by URL.
fn publish(seq: u64, plan: &[u8], assets: &[(&str, &[u8])]) -> Vec<(String, Vec<u8>)> {
    let head = serde_json::json!({
        "exact": 1, "app": {"id": "com.exact.t"},
        "plan": {"url": "./app.plan", "sha256": sha256_hex(plan), "bytes": plan.len()},
        "assets": assets.iter().map(|(name, bytes)| serde_json::json!({
            "name": name, "url": format!("./{name}"),
            "sha256": sha256_hex(bytes), "bytes": bytes.len(),
        })).collect::<Vec<_>>(),
        "stream": {"channel": "prod", "compatibilityId": "abc", "seq": seq},
    });
    let mut files = vec![
        (
            format!("{BASE}/exact.json"),
            serde_json::to_vec(&head).unwrap(),
        ),
        (format!("{BASE}/app.plan"), plan.to_vec()),
    ];
    files.extend(
        assets
            .iter()
            .map(|(name, bytes)| (format!("{BASE}/{name}"), bytes.to_vec())),
    );
    files
}

/// One check over `files`; returns its outcome and the last segment of each
/// URL fetched, in order.
fn check(client: &mut Client, files: &[(String, Vec<u8>)]) -> (Outcome, Vec<String>) {
    let mut asked = Vec::new();
    let outcome = client.check(&mut |url| {
        asked.push(url.rsplit('/').next().unwrap().to_string());
        files
            .iter()
            .find(|(u, _)| u == url)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| format!("404 {url}"))
    });
    (outcome, asked)
}

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows durable store is not qualified (LLP 1026 D11a)"
)]
fn the_first_update_takes_unchanged_assets_from_the_bundle() {
    let base = Temp::new("client-embedded-base");
    let bundle = Temp::new("client-embedded-bundle");
    let assets: [(&str, &[u8]); 3] = [
        ("assets/a1.png", b"one"),
        ("assets/a2.png", b"two"),
        ("assets/a3.png", b"three"),
    ];
    for (name, bytes) in assets {
        let path = bundle.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    // The bundle's a3 has the head's length but other bytes: its name is
    // not enough, so it is fetched.
    std::fs::write(bundle.path().join("assets/a3.png"), b"THREE").unwrap();
    let mut client = Client::open_at(
        base.path(),
        bundle.path(),
        COMPAT,
        b"plan v1",
        Some(CLIENT_ORIGIN),
    )
    .unwrap();
    let (outcome, asked) = check(&mut client, &publish(1, b"plan v2", &assets));
    assert_eq!(
        (outcome.to_string(), asked),
        (
            "staged seq 1".to_string(),
            vec!["exact.json".into(), "app.plan".into(), "a3.png".into()]
        )
    );
    let candidate = client.prepare_activation().unwrap().unwrap();
    for (name, bytes) in assets {
        assert_eq!(
            candidate.assets.resolve(name).unwrap().as_deref(),
            Some(bytes),
            "{name}"
        );
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "Windows durable store is not qualified (LLP 1026 D11a)"
)]
fn an_embedded_file_changed_after_it_was_hashed_is_fetched() {
    let temp = Temp::new("reuse-stale-digest");
    let bundle = Temp::new("reuse-stale-digest-bundle");
    let path = bundle.path().join("mark.png");
    // The host hashed "a mark"; the file now holds other bytes of that length.
    std::fs::write(&path, b"A MARK").unwrap();
    let mut origin = Origin::of(&Bundle::new(4, b"plan four").asset("mark.png", b"a mark"));
    let mut store = open(&temp);
    let files = &origin.files;
    let asked = &mut origin.asked;
    let mut fetch = |url: &str| -> Result<Vec<u8>, String> {
        asked.push(url.rsplit('/').next().unwrap().to_string());
        files.get(url).cloned().ok_or_else(|| format!("404 {url}"))
    };
    let mut embedded = |_: &str| {
        Some(EmbeddedAsset {
            sha256: sha256_hex(b"a mark"),
            path: path.clone(),
        })
    };
    let head = exact_update::head_url(ORIGIN, "release", COHORT);
    let Ok(Check::Staged { .. }) = store.check(&head, &mut fetch, &mut embedded) else {
        panic!("the head should have staged");
    };
    assert_eq!(origin.asked, ["exact.json", "app.plan", "mark.png"]);
    let candidate = store.prepare_activation().unwrap().unwrap();
    assert_eq!(
        candidate.assets.resolve("mark.png").unwrap().as_deref(),
        Some(b"a mark".as_slice())
    );
}
