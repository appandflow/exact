use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// A plan pushed to an installed cohort is described by its own format, so a
/// cohort whose binary reads another format is refused, not given a plan it
/// cannot decode.
#[test]
fn a_plan_graph_names_the_plans_own_format() {
    let plan = exact_plan::builder::PlanBuilder::new(7, 0)
        .finish()
        .unwrap()
        .encode();
    let inputs = json!({"formatVersion": 1, "formatDigest": "0000000000000001", "kernelSchema": "0000000000000002"});
    let graph = plan_graph(&plan, &inputs, "aarch64-apple-darwin").unwrap();
    let requires = &graph["artifacts"][0]["requires"];
    assert_eq!(requires["formatVersion"], json!(exact_plan::FORMAT_VERSION));
    assert_eq!(
        requires["formatDigest"],
        json!(format!("{:016x}", exact_plan::FORMAT_DIGEST))
    );
    assert_eq!(requires["kernelSchema"], json!("0000000000000007"));
}

#[test]
fn loopback_callback_ports_retain_the_listener() {
    let plan = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap()
        .encode();
    for callback in [
        "http://127.0.0.1/auth/callback",
        "http://127.0.0.1:1455/auth/callback",
        "http://[::1]:1455/auth/callback",
    ] {
        let inputs = json!({"grantCeiling": format!("auth.callback {callback}")});
        let graph = plan_graph(&plan, &inputs, "aarch64-apple-darwin").unwrap();
        assert!(
            graph["loads"]
                .as_array()
                .unwrap()
                .contains(&json!("listen")),
            "{callback}"
        );
    }
    assert!(!listens(
        &json!({"grantCeiling": "auth.callback https://example.com/auth/callback"})
    ));
}

/// LLP 1012.001.000 D6: a development bake leaves its plan's map beside
/// it, keyed by that plan's digest; a production bake leaves none, and a
/// plan whose nodes the contract no longer compiles to gets none.
#[test]
fn a_development_bake_leaves_its_plans_source_map() {
    let root = std::env::temp_dir().join(format!("exact-map-{}", std::process::id()));
    let (app, out) = (root.join("app"), root.join("out"));
    std::fs::create_dir_all(&app).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        app.join("app.contract"),
        "component App\n  view\n    text \"a\" testId=\"a\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&app.join("app.contract")).unwrap();
    let bytes = plan.encode();
    let map = out.join("app.plan.map.json");
    source_map("development", &app, &out, &plan, &bytes);
    let written: Value = serde_json::from_slice(&std::fs::read(&map).unwrap()).unwrap();
    assert_eq!(written["digest"], contract::plan_digest(&bytes));
    source_map("production", &app, &out, &plan, &bytes);
    assert!(!map.exists(), "a production bake carries no map");
    let other = contract::compile("component App\n  view\n    column\n      text \"b\"\n").unwrap();
    source_map("development", &app, &out, &other, &other.encode());
    assert!(!map.exists(), "a map for other nodes is never written");
    std::fs::remove_dir_all(&root).unwrap();
}

/// LLP 1096 D8: a declared sound is not a node, so the plan's `sounds`
/// table is what says a fixed build carries the sound arm.
#[test]
fn a_plan_with_a_sound_loads_the_sound_arm() {
    let without = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap();
    let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
    b.sound("assets/kick.wav", 4_800, 48_000, 1, "00");
    let with = b.finish().unwrap();
    for (plan, loads) in [(without, json!([])), (with, json!(["sound"]))] {
        let graph = artifact_graph(&plan, &json!({}), &[], false, "aarch64-apple-darwin").unwrap();
        assert_eq!(graph["loads"], loads);
    }
}

/// LLP 1042 §9: an image's picture in picture is the video arm's, so a
/// plan that asks for it loads the arm with no `video` node.
#[test]
fn a_plan_that_floats_an_image_loads_the_video_arm() {
    let plan = contract::compile(
            "component App\n  action float\n    requestPictureInPicture(\"page\")\n  view\n    column\n      button press=float\n        text \"Float\"\n      image \"assets/a.png\" id=\"page\" width=1 height=1\n",
        )
        .unwrap();
    let graph = artifact_graph(&plan, &json!({}), &[], false, "aarch64-apple-darwin").unwrap();
    assert_eq!(graph["loads"], json!(["video"]));
}

/// LLP 1075.003.000.001 §4.3: the plan requires the hatch words it marks
/// that this platform's module handles, and no others.
#[test]
fn a_plan_requires_the_hatch_words_its_platform_handles() {
    let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
    for word in ["dot", "avatar", "avatar", "badge"] {
        let expr = b.constant(&exact_plan::Value::str(word));
        let bindings = [exact_plan::BindingsRow {
            kind: exact_plan::BindingKind::Prop,
            id: exact_kernel::PropId::Hatch as u16,
            expr,
            written: true,
        }];
        b.node(
            exact_kernel::NodeType::Video as u8,
            None,
            None,
            0,
            &bindings,
            &[],
            None,
        );
    }
    let plan = b.finish().unwrap();
    let requires = |inputs: Value| {
        let graph = artifact_graph(&plan, &inputs, &[], false, "aarch64-apple-darwin").unwrap();
        graph["artifacts"][0]["requires"]["hatches"].clone()
    };
    // This platform handles `avatar` and `dot`; `badge` is another platform's.
    assert_eq!(
        requires(json!({"hatches": ["avatar", "dot", "unused"]})),
        json!(["avatar", "dot"])
    );
    // A platform that handles none of them is asked for nothing.
    assert_eq!(requires(json!({"hatches": null})), Value::Null);
    assert_eq!(requires(json!({})), Value::Null);
}

/// LLP 1108 D6 R2: a TypeScript app's plan requires the prelude runtime
/// its compatibility inputs carry (`exactBodyFrom`), so an older cohort
/// takes no bundle; an app without one requires none.
#[test]
fn a_plan_requires_the_typescript_runtime_its_inputs_carry() {
    let plan = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap();
    let requires = |inputs: Value| {
        let graph = artifact_graph(&plan, &inputs, &[], false, "aarch64-apple-ios").unwrap();
        graph["artifacts"][0]["requires"]["typescriptRuntime"].clone()
    };
    assert_eq!(
        requires(json!({"typescriptRuntime": crate::compat::TYPESCRIPT_RUNTIME})),
        json!(crate::compat::TYPESCRIPT_RUNTIME)
    );
    assert_eq!(requires(json!({})), Value::Null);
}

/// LLP 1098 D8: the receipt says whether a node claims the media session.
#[test]
fn a_plan_with_metadata_claims_the_media_session() {
    for (prop, claims) in [
        (exact_kernel::PropId::Src, false),
        (exact_kernel::PropId::MediaTitle, true),
    ] {
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        let expr = b.constant(&exact_plan::Value::str("Episode 1"));
        let bindings = [exact_plan::BindingsRow {
            kind: exact_plan::BindingKind::Prop,
            id: prop as u16,
            expr,
            written: true,
        }];
        b.node(
            exact_kernel::NodeType::Video as u8,
            None,
            None,
            0,
            &bindings,
            &[],
            None,
        );
        let graph = artifact_graph(
            &b.finish().unwrap(),
            &json!({}),
            &[],
            false,
            "aarch64-apple-darwin",
        )
        .unwrap();
        assert_eq!(
            (graph["loads"].clone(), graph["mediaSession"].clone()),
            (json!(["video"]), json!(claims))
        );
    }
}

/// LLP 1116.011 D2.3: the receipt says what the first route stands on, for
/// the iOS launch screen: the grouped grey where the route scrolls a
/// grouped list (stopwatch, round 13), the page's own otherwise.
#[test]
fn a_plan_says_the_ground_its_first_route_stands_on() {
    const SCREEN: &str = "routes nav
  home \"/\"
component App
  view
    main navigationKey=`${top(nav).id}` navigationBack=\"back\"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll=\"l\" ROUTE position=\"absolute\" inset=0
          header
            text \"Laps\" role=\"heading\" aria-level=1
          LIST
            section
              row
                text \"One\"
";
    const GROUPED: &str =
        "list id=\"l\" appearance=\"auto\" listStyle=\"inset-grouped\" flex=1 min-height=0";
    let launch = |route: &str, list: &str| {
        let plan = contract::compile(&SCREEN.replace("ROUTE", route).replace("LIST", list))
            .unwrap_or_else(|e| panic!("{e}"));
        artifact_graph(&plan, &json!({}), &[], false, "aarch64-apple-ios-sim").unwrap()
            ["launchBackground"]
            .clone()
    };
    assert_eq!(launch("", GROUPED), json!("grouped"));
    assert_eq!(
        launch("", &GROUPED.replace("inset-grouped", "grouped")),
        json!("grouped")
    );
    // Any other scroll, or a page of its own, is the platform's.
    assert_eq!(
        launch("", "scroll id=\"l\" flex=1 min-height=0"),
        json!("system")
    );
    assert_eq!(
        launch("background-color=\"-exact-background\"", GROUPED),
        json!("system")
    );
    // No route at all, as a plan with no navigation root is.
    let bare = contract::compile("component App\n  view\n    text \"a\" testId=\"a\"\n").unwrap();
    assert_eq!(
        artifact_graph(&bare, &json!({}), &[], false, "aarch64-apple-ios-sim").unwrap()
            ["launchBackground"],
        json!("system")
    );
}

#[test]
fn a_no_delivery_development_bake_names_its_gpu_dev_module() {
    // Games are store L=0: the cohort has no trust input, yet the
    // host-less proof's split host bake must name the gpu-dev module.
    let card = gpu_card(
        "com.example.game",
        "c",
        "development",
        Some("libg.so".into()),
        None,
    );
    assert_eq!(card.as_ref().unwrap()["name"], "libg.so");
    assert_eq!(card.as_ref().unwrap()["receipt"], true);
    let product = Some(("libg.so".into(), "ab".into()));
    let release = gpu_card(
        "com.example.game",
        "c",
        "production",
        Some("libg.so".into()),
        product,
    );
    assert_eq!(release.as_ref().unwrap()["sha256"], "ab");
    assert_eq!(release.unwrap()["trust"], "production");
    assert!(gpu_card(
        "com.example.game",
        "c",
        "production",
        Some("libg.so".into()),
        None
    )
    .is_none());
}

fn rust_fixture(plan: &[u8]) -> (PathBuf, Value, Value) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "exact-rust-bake-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // Parallel tests can observe the same clock tick. Each fixture owns
    // its directory exclusively, including cleanup after staging.
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("input")).unwrap();
    let inputs =
        json!({"app":"com.exact.caltrain","grantCeiling":"","rustGrants":"","rustMode":"wasm"});
    let module = b"\0asm\x01\0\0\0";
    let receipt = json!({"version":1,"kind":"rust","abi":crate::compat::RUST_ABI,"appId":inputs["app"],"grants":"",
            "target":"wasm32-unknown-unknown","executor":"wasm",
            "plan":{"file":"app.plan","sha256":hash(plan),"bytes":plan.len()},
            "module":{"file":"app.module.wasm","sha256":hash(module),"bytes":module.len()}});
    std::fs::write(root.join("input/app.module.wasm"), module).unwrap();
    std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
    (root, inputs, receipt)
}

#[test]
fn supplemental_rust_is_real_paired_bytes_with_executor_requirements() {
    let mut plan = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap();
    plan.app_id = "com.exact.caltrain".into();
    let bytes = plan.encode();
    let (root, inputs, _) = rust_fixture(&bytes);
    let cards = stage_rust_bundle(
        &root.join("input"),
        &root.join("out"),
        &bytes,
        &inputs,
        "aarch64-apple-ios",
    )
    .unwrap();
    assert_eq!(cards.len(), 2);
    for card in &cards {
        let actual = std::fs::read(root.join("out").join(card["name"].as_str().unwrap())).unwrap();
        assert_eq!(card["sha256"], hash(&actual));
        assert_eq!(card["bytes"], actual.len());
    }
    let graph = artifact_graph(&plan, &inputs, &cards, false, "aarch64-apple-ios").unwrap();
    for row in graph["artifacts"].as_array().unwrap() {
        assert_eq!(row["requires"]["rustAbi"], 2);
        assert_eq!(row["requires"]["rustMode"], "wasm");
        assert_eq!(row["requires"]["rustTarget"], "wasm32-unknown-unknown");
    }
    assert!(graph["artifacts"][0]["requires"].get("sources").is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn tiered_assets_bind_the_complete_container_and_exact_native_cohort() {
    let mut plan = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap();
    plan.app_id = "com.exact.caltrain".into();
    let bytes = plan.encode();
    let (root, mut inputs, mut receipt) = rust_fixture(&bytes);
    let target = "aarch64-apple-darwin";
    let module = b"EXLT\x01\0\0\0\x08\0\0\0\0asm\x01\0\0\0native";
    inputs["rustMode"] = json!("tiered");
    receipt["executor"] = json!("tiered");
    receipt["target"] = json!(target);
    receipt["module"] = json!({"file":"app.module.bin","sha256":hash(module),"bytes":module.len()});
    std::fs::write(root.join("input/app.module.bin"), module).unwrap();
    std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
    let stage = |host| {
        stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            &bytes,
            &inputs,
            host,
        )
    };
    let cards = stage(target).unwrap();
    let graph = artifact_graph(&plan, &inputs, &cards, false, target).unwrap();
    for row in graph["artifacts"].as_array().unwrap() {
        assert_eq!(row["requires"]["rustMode"], "tiered");
        assert_eq!(row["requires"]["rustTarget"], target);
    }
    for incompatible in [
        "x86_64-apple-darwin",
        "aarch64-apple-ios",
        "wasm32-unknown-unknown",
    ] {
        assert!(stage(incompatible).is_err(), "{incompatible}");
    }
    // A changed native companion is covered by the same authenticated digest.
    let mut changed = module.to_vec();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(root.join("input/app.module.bin"), changed).unwrap();
    assert!(stage(target).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn supplemental_rust_refuses_policy_identity_grants_target_and_tampered_pair() {
    let plan = b"exact baked plan";
    let (root, mut inputs, receipt) = rust_fixture(plan);
    for (key, value) in [
        ("appId", json!("another.app")),
        ("grants", json!("secret.keep token")),
        ("abi", json!(99)),
        ("kind", json!("javascript")),
        ("target", json!("aarch64-apple-ios")),
        ("executor", json!("native")),
    ] {
        let mut changed = receipt.clone();
        changed[key] = value;
        std::fs::write(root.join("input/app.module.json"), changed.to_string()).unwrap();
        assert!(
            stage_rust_bundle(
                &root.join("input"),
                &root.join("out"),
                plan,
                &inputs,
                "aarch64-apple-ios"
            )
            .is_err(),
            "{key}"
        );
        assert!(!root.join("out/rust").exists());
    }
    std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
    inputs["rustMode"] = json!("off");
    assert!(stage_rust_bundle(
        &root.join("input"),
        &root.join("out"),
        plan,
        &inputs,
        "aarch64-apple-ios"
    )
    .is_err());
    inputs["rustMode"] = json!("wasm");
    assert!(stage_rust_bundle(
        &root.join("input"),
        &root.join("out"),
        b"changed plan",
        &inputs,
        "aarch64-apple-ios"
    )
    .is_err());
    std::fs::write(root.join("input/app.module.wasm"), b"changed module").unwrap();
    assert!(stage_rust_bundle(
        &root.join("input"),
        &root.join("out"),
        plan,
        &inputs,
        "aarch64-apple-ios"
    )
    .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mixed_rust_receipt_preserves_its_own_scope_under_the_union_ceiling() {
    let plan = b"mixed baked plan";
    let (root, mut inputs, mut receipt) = rust_fixture(plan);
    inputs["grantCeiling"] = json!("fs.read app:/data/rust\nnet.fetch https://example.test/");
    inputs["rustGrants"] = json!("fs.read app:/data/rust");
    inputs["javascriptGrants"] = json!("net.fetch https://example.test/");
    receipt["grants"] = inputs["rustGrants"].clone();
    std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
    assert!(stage_rust_bundle(
        &root.join("input"),
        &root.join("out"),
        plan,
        &inputs,
        "aarch64-apple-ios"
    )
    .is_ok());
    receipt["grants"] = inputs["grantCeiling"].clone();
    std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
    assert!(
        stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            plan,
            &inputs,
            "aarch64-apple-ios"
        )
        .is_err(),
        "a module cannot borrow its sibling's grant"
    );
    receipt["grants"] = inputs["rustGrants"].clone();
    std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
    inputs["grantCeiling"] = inputs["javascriptGrants"].clone();
    assert!(
        stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            plan,
            &inputs,
            "aarch64-apple-ios"
        )
        .is_err(),
        "source grants must fit the host ceiling"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn signed_release_requires_the_complete_real_supplemental_roster() {
    let (mut compat, mut receipt, plan) = signed_fixture();
    let (root, inputs, _) = rust_fixture(&plan);
    let cards = stage_rust_bundle(
        &root.join("input"),
        &root.join("out"),
        &plan,
        &inputs,
        "aarch64-apple-ios",
    )
    .unwrap();
    for card in &cards {
        receipt["envelope"]["assets"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "name":card["name"],"sha256":card["sha256"],"bytes":card["bytes"],
                "url":format!("../../blobs/{}",card["sha256"].as_str().unwrap())}));
    }
    // A fresh process-local test key: no publisher's private key is stored.
    let canonical = exact_update::canonical_bytes(&receipt["envelope"].to_string()).unwrap();
    let output = crate::bun().args(["-e", r#"
            const {generateKeyPairSync,sign}=require('node:crypto');
            const {publicKey,privateKey}=generateKeyPairSync('ed25519');
            process.stdout.write(JSON.stringify({public:publicKey.export({type:'spki',format:'der'}).subarray(-32).toString('base64'),
                signature:sign(null,Buffer.from(process.argv[1]),privateKey).toString('base64')}));
        "#]).arg(String::from_utf8(canonical).unwrap()).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let signing: Value = serde_json::from_slice(&output.stdout).unwrap();
    compat.inputs["keys"] = json!({"fixture":signing["public"]});
    receipt["envelope"]["signature"] = json!({"keyId":"fixture","ed25519":signing["signature"]});
    let receipt = serde_json::to_vec(&receipt).unwrap();
    assert!(apply_release(&mut compat, &receipt, &plan)
        .unwrap_err()
        .contains("complete embedded asset roster"));
    let assets = compat.embedded["assets"].as_array_mut().unwrap();
    assets.extend(cards);
    assets.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    apply_release(&mut compat, &receipt, &plan).unwrap();
    assert_eq!(compat.embedded["seq"], 1);
    std::fs::remove_dir_all(root).unwrap();
}

/// LLP 1141.001 D1: a bundled SVG image whose art is under a `mask`
/// loads the island module, though the plan has no `mask`; the same
/// logo without one, an SVG under another name and a PNG load nothing.
#[test]
#[cfg(unix)]
fn a_masked_svg_image_loads_the_svg_island_module() {
    let root = std::env::temp_dir().join(format!(
        "exact-bake-svg-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("assets/sponsors")).unwrap();
    let art = "<path d=\"M0 0H5V5H0Z\" fill=\"#F2545B\"/>";
    let masked = format!(
            "<svg width=\"24\" height=\"12\" viewBox=\"0 0 24 12\" xmlns=\"http://www.w3.org/2000/svg\">\
             <mask id=\"m\" style=\"mask-type:luminance\" maskUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"24\" height=\"12\">\
             <path d=\"M24 0H0V12H24V0Z\" fill=\"white\"/></mask><g mask=\"url(#m)\">{art}</g></svg>"
        );
    let plain =
        format!("<svg viewBox=\"0 0 24 12\" xmlns=\"http://www.w3.org/2000/svg\">{art}</svg>");
    std::fs::write(root.join("assets/sponsors/plain.svg"), &plain).unwrap();
    std::fs::write(root.join("assets/icon.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let plan = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap();
    let manifest = Manifest::read(&root).unwrap();
    let loads = |root: &Path| {
        let cards = asset_cards(root, &root.join("out"), &manifest).unwrap();
        let pictures = asset_islands(root, &cards);
        artifact_graph(&plan, &json!({}), &cards, pictures, "aarch64-apple-ios").unwrap()["loads"]
            .clone()
    };
    assert_eq!(loads(&root), json!([]));
    // The host sniffs the bytes, so a masked SVG named `.png` counts too.
    std::fs::write(root.join("assets/sponsors/logo.png"), &masked).unwrap();
    assert_eq!(loads(&root), json!(["svg"]));
    std::fs::remove_file(root.join("assets/sponsors/logo.png")).unwrap();
    std::fs::write(root.join("assets/sponsors/gold.svg"), &masked).unwrap();
    assert_eq!(loads(&root), json!(["svg"]));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn the_embedded_roster_uses_the_copy_gate_and_refuses_symlink_assets() {
    let root = std::env::temp_dir().join(format!(
        "exact-bake-assets-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::create_dir_all(root.join("deck/nested")).unwrap();
    std::fs::write(root.join("assets/image.png"), b"image").unwrap();
    std::fs::write(root.join("deck/nested/index.html"), b"deck").unwrap();
    let manifest = Manifest::read(&root).unwrap();
    let cards = asset_cards(&root, &root.join("out"), &manifest).unwrap();
    assert_eq!(
        cards,
        vec![
            json!({"name":"assets/image.png","sha256":hash(b"image"),"bytes":5}),
            json!({"name":"deck/nested/index.html","sha256":hash(b"deck"),"bytes":4}),
        ]
    );
    std::fs::write(root.join("assets/two words.png"), b"space").unwrap();
    assert!(asset_cards(&root, &root.join("out"), &manifest)
        .unwrap_err()
        .contains("bake asset gate: the asset name assets/two words.png is not portable"));
    std::fs::remove_file(root.join("assets/two words.png")).unwrap();
    std::fs::write(root.join("outside"), b"must not be embedded").unwrap();
    std::os::unix::fs::symlink("../outside", root.join("assets/escape")).unwrap();
    assert!(asset_cards(&root, &root.join("out"), &manifest)
        .unwrap_err()
        .contains("bake asset gate"));
    std::fs::remove_dir_all(root).unwrap();
}

fn signed_fixture() -> (Compat, Value, Vec<u8>) {
    let raw = include_bytes!("../../../update/tests/it/fixtures/publisher/exact.json");
    let head = Envelope::parse(raw).unwrap();
    let plan = include_bytes!("../../../update/tests/it/fixtures/publisher/app.plan").to_vec();
    let key = include_str!("../../../update/tests/it/fixtures/publisher/caltrain-2026.pub").trim();
    let mut assets = head
        .assets
        .iter()
        .map(|a| json!({"name":a.name,"sha256":a.sha256,"bytes":a.bytes}))
        .collect::<Vec<_>>();
    assets.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let compat = Compat {
        id: head.stream.compatibility_id.clone(),
        inputs: json!({"app":head.app_id,"trust":"production","keys":{"caltrain-2026":key},"store":{"L":"A"}}),
        channel: head.stream.channel.clone(),
        origin: None,
        activate: "next-launch".into(),
        target: "aarch64-apple-ios".into(),
        reach: Value::Null,
        painter: None,
        game: false,
        embedded: json!({"seq":0,"genesis":true,"entryDigest":null,"plan":{"sha256":head.plan.sha256,"bytes":head.plan.bytes},"assets":assets}),
    };
    (
        compat,
        json!({"envelope":serde_json::from_slice::<Value>(raw).unwrap()}),
        plan,
    )
}

#[test]
fn a_production_sequence_is_bound_to_a_signed_matching_complete_bundle() {
    let (mut compat, receipt, plan) = signed_fixture();
    apply_release(&mut compat, &serde_json::to_vec(&receipt).unwrap(), &plan).unwrap();
    assert_eq!(compat.embedded["seq"], 1);
    assert_eq!(compat.embedded["genesis"], false);
    assert_eq!(compat.embedded["entryDigest"].as_str().unwrap().len(), 64);
    let baked = Baked::from_compat(&compat.to_json(), &plan).unwrap();
    assert_eq!(baked.embedded.compatibility_id, compat.id);
    assert_eq!(baked.embedded.seq, 1);

    let (mut wrong, _, _) = signed_fixture();
    wrong.embedded["assets"] = json!([]);
    assert!(
        apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan)
            .unwrap_err()
            .contains("complete embedded asset roster")
    );
    let (mut wrong, _, _) = signed_fixture();
    wrong.embedded["plan"]["sha256"] = json!("0".repeat(64));
    assert!(
        apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan)
            .unwrap_err()
            .contains("embedded.plan does not match")
    );
    let (mut wrong, mut forged, _) = signed_fixture();
    forged["envelope"]["stream"]["seq"] = json!(41);
    assert!(apply_release(&mut wrong, &serde_json::to_vec(&forged).unwrap(), &plan).is_err());
    let (mut wrong, _, _) = signed_fixture();
    wrong.channel = "another".into();
    assert!(apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan).is_err());
}
