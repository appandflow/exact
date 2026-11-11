// Body and collider controls that are saved only when set: a force, an authored
// inertia, average restitution.
use exact_game::{bin, hash, json, Entity, Transform, Vec3, World};
use exact_game_physics::{self as physics, Body, Collider, Shape};

fn world() -> World {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w
}
fn restored(w: &World) -> World {
    let mut r = world();
    r.load(&w.save()).unwrap();
    r
}
fn ball(w: &mut World, x: f32, body: Body) -> Entity {
    w.spawn((
        Transform::at(x, 0.0, 0.0),
        body,
        Collider {
            shape: Shape::Sphere { radius: 0.5 },
            ..Collider::default()
        },
    ))
}

// A force accelerates its body by F/m every tick until it is written back to zero,
// contacts still act on the body it pushes, and a restored world continues with
// the same force to the bit.
#[test]
fn center_force_persists_until_cleared_and_replays() {
    let mut w = world();
    let free = Body {
        gravity: 0.0,
        mass: 2.0,
        ..Body::default()
    };
    let pushed = ball(
        &mut w,
        0.0,
        Body {
            force: Vec3::new(4.0, 0.0, 0.0),
            ..free.clone()
        },
    );
    let other = ball(&mut w, 1.5, free.clone());
    let speed = |w: &World, e| w.get::<Body>(e).unwrap().velocity.x;
    for _ in 0..30 {
        physics::step(&mut w);
    }
    assert_eq!(physics::mass(&w, pushed), Some(2.0));
    // 2 m/s² for half a second; the balls have not met.
    assert!(
        (speed(&w, pushed) - 1.0).abs() < 1e-4,
        "{}",
        speed(&w, pushed)
    );
    assert_eq!(speed(&w, other), 0.0);
    let mut r = restored(&w);
    assert_eq!(r.hash(), w.hash());
    for tick in 0..90 {
        physics::step(&mut w);
        physics::step(&mut r);
        assert_eq!(r.hash(), w.hash(), "tick {tick}");
    }
    assert_eq!(
        r.get::<Body>(pushed).unwrap().force,
        Vec3::new(4.0, 0.0, 0.0)
    );
    // They touched: the force moves both, so their momentum is the force's impulse.
    let momentum = 2.0 * (speed(&w, pushed) + speed(&w, other));
    assert!(speed(&w, other) > 0.0);
    assert!((momentum - 4.0 * 2.0).abs() < 1e-3, "{momentum}");
    w.get_mut::<Body>(pushed).unwrap().force = Vec3::ZERO;
    physics::step(&mut w);
    let total = speed(&w, pushed) + speed(&w, other);
    for _ in 0..30 {
        physics::step(&mut w);
    }
    let after = speed(&w, pushed) + speed(&w, other);
    assert!((after - total).abs() < 1e-5, "{total} then {after}");
}

// The new fields are written only when set (so existing saves and hashes keep their
// bytes), round-trip through JSON, binary and a world save when set, keep their
// value through a patch that omits them, and enter the hash under their names.
#[test]
fn optional_force_and_inertia_round_trip() {
    let set = Body {
        force: Vec3::new(1.0, 2.0, 3.0),
        inertia: Vec3::new(0.5, 0.25, 0.125),
        ..Body::default()
    };
    let plain = json::to_string(&Body::default()).unwrap();
    assert!(
        !plain.contains("force") && !plain.contains("inertia"),
        "{plain}"
    );
    let text = json::to_string(&set).unwrap();
    assert!(
        text.contains("\"force\":[1.0,2.0,3.0]") && text.contains("\"inertia\":[0.5,0.25,0.125]"),
        "{text}"
    );
    assert_eq!(json::from_str::<Body>(&text).unwrap(), set);
    assert_eq!(bin::from_slice::<Body>(&bin::to_vec(&set)).unwrap(), set);
    let mut patched = set.clone();
    json::read_into(r#"{"damping":0.1}"#, &mut patched).unwrap();
    assert_eq!((patched.force, patched.inertia), (set.force, set.inertia));
    let force = Body {
        force: Vec3::ONE,
        ..Body::default()
    };
    let inertia = Body {
        inertia: Vec3::ONE,
        ..Body::default()
    };
    assert_ne!(hash::of(&force), hash::of(&inertia));
    let average = Collider {
        bounce_average: true,
        ..Collider::default()
    };
    assert!(!json::to_string(&Collider::default())
        .unwrap()
        .contains("bounce_average"));
    assert!(json::to_string(&average)
        .unwrap()
        .contains("\"bounce_average\":true"));
    assert_eq!(
        bin::from_slice::<Collider>(&bin::to_vec(&average)).unwrap(),
        average
    );
    for (body, collider) in [(Body::default(), Collider::default()), (set, average)] {
        let mut w = world();
        let e = w.spawn((Transform::default(), body.clone(), collider.clone()));
        physics::step(&mut w);
        let mut r = restored(&w);
        assert_eq!(r.get::<Body>(e).unwrap().force, body.force);
        assert_eq!(r.get::<Body>(e).unwrap().inertia, body.inertia);
        assert_eq!(*r.get::<Collider>(e).unwrap(), collider);
        assert_eq!(r.hash(), w.hash());
        physics::step(&mut w);
        physics::step(&mut r);
        assert_eq!(r.hash(), w.hash());
    }
}

// A drop's rebound speed over its impact speed is the pair's restitution: the
// larger of the two, or their average when both colliders ask for it.
#[test]
fn bounce_average_needs_both_colliders() {
    for (ball_average, floor_average, want) in [
        (false, false, 0.42),
        (true, false, 0.42),
        (false, true, 0.42),
        (true, true, 0.21),
    ] {
        let mut w = World::new(120, 0);
        physics::register(&mut w);
        w.spawn((
            Transform::at(0.0, -0.5, 0.0),
            Collider {
                shape: Shape::Box {
                    half: Vec3::new(5.0, 0.5, 5.0),
                },
                bounce_average: floor_average,
                ..Collider::default()
            },
        ));
        let e = w.spawn((
            Transform::at(0.0, 1.0, 0.0),
            Body::default(),
            Collider {
                shape: Shape::Sphere { radius: 0.1 },
                bounce: 0.42,
                bounce_average: ball_average,
                ..Collider::default()
            },
        ));
        let (mut impact, mut rebound) = (0.0f32, 0.0f32);
        for _ in 0..120 {
            physics::step(&mut w);
            let v = w.get::<Body>(e).unwrap().velocity.y;
            if rebound == 0.0 {
                impact = impact.min(v);
            }
            rebound = rebound.max(v);
        }
        let ratio = rebound / -impact;
        assert!(
            (ratio - want).abs() < 0.02,
            "{ball_average} {floor_average}: {ratio}"
        );
    }
}

// A die's extra shape meets other dice while its primary meets the table: each
// filters by its own layers, adds no mass, is invisible to queries, reports its
// touches as the die (one begin and one end per entity pair, however many of its
// shapes touch), is saved and restored with the world, and goes when removed.
#[test]
fn extra_colliders_share_the_body_and_filter_by_their_own_layers() {
    let mut w = world();
    let floor = w.spawn((
        Transform::at(0.0, -0.5, 0.0),
        Collider {
            shape: Shape::Box {
                half: Vec3::new(20.0, 0.5, 20.0),
            },
            friction: 0.0,
            layer: 3,
            ..Collider::default()
        },
    ));
    let die = w.spawn((
        Transform::at(0.0, 0.5, 0.0),
        Body {
            velocity: Vec3::X * 2.0,
            ..Body::default()
        },
        Collider {
            shape: Shape::Sphere { radius: 0.5 },
            friction: 0.0,
            mask: 1,
            extra: vec![Collider {
                shape: Shape::Box {
                    half: Vec3::splat(0.4),
                },
                offset: Vec3::new(0.0, -0.1, 0.0),
                friction: 0.0,
                layer: 2,
                mask: 2,
                ..Collider::default()
            }],
            ..Collider::default()
        },
    ));
    let other = w.spawn((
        Transform::at(1.5, 0.2, 0.0),
        Body::default(),
        Collider {
            shape: Shape::Box {
                half: Vec3::splat(0.2),
            },
            friction: 0.0,
            layer: 2,
            mask: 2,
            ..Collider::default()
        },
    ));
    let x = |w: &World, e| w.get::<Transform>(e).unwrap().position.x;
    let pair = |a: Entity, b: Entity| (a.min(b), a.max(b));
    let mut log = Vec::new();
    let tick = |w: &mut World, log: &mut Vec<(usize, Entity, Entity, bool)>, n: usize| {
        physics::step(w);
        log.extend(physics::events(w).iter().map(|t| (n, t.a, t.b, t.began)));
    };
    for n in 0..20 {
        tick(&mut w, &mut log, n);
    }
    // The primary sphere's mass alone.
    let sphere = 1000.0 * 4.0 / 3.0 * std::f32::consts::PI * 0.125;
    let mass = physics::mass(&w, die).unwrap();
    assert!((mass - sphere).abs() < 1e-3 * sphere, "{mass}");
    // A ray through the extra box alone, beside the sphere, misses the die.
    let low = Vec3::new(x(&w, die) + 0.39, 0.05, 0.39);
    let hit = physics::raycast(&w, low + Vec3::Y, -Vec3::Y, 2.0, u32::MAX).unwrap();
    assert_eq!(hit.entity, floor);
    let mut r = restored(&w);
    let mut replay = Vec::new();
    for n in 20..60 {
        tick(&mut w, &mut log, n);
        tick(&mut r, &mut replay, n);
        assert_eq!(r.hash(), w.hash(), "tick {n}");
    }
    assert_eq!(
        replay,
        log.iter()
            .filter(|t| t.0 >= 20)
            .copied()
            .collect::<Vec<_>>()
    );
    assert!(x(&w, other) > 1.6, "the box pushed the other die");
    // Sphere and box both rest on the floor: one begin, at the first step.
    let (a, b) = pair(die, floor);
    assert_eq!(log.iter().filter(|t| (t.1, t.2) == (a, b)).count(), 1);
    assert!(log.contains(&(0, a, b, true)), "{log:?}");
    // The other die is met by the box, not the sphere, which reaches it first.
    let (a, b) = pair(die, other);
    let met = log.iter().find(|t| (t.1, t.2) == (a, b)).expect("met").0;
    assert!(2.0 * (met + 1) as f32 / 60.0 > 0.85, "met at tick {met}");
    // Removing the extras ends that touch; the sphere passes through the other
    // die, stopped where it stands.
    w.get_mut::<Collider>(die).unwrap().extra.clear();
    w.get_mut::<Body>(other).unwrap().velocity = Vec3::ZERO;
    tick(&mut w, &mut log, 60);
    assert_eq!(*log.last().unwrap(), (60, a, b, false));
    for n in 61..120 {
        tick(&mut w, &mut log, n);
    }
    assert!(x(&w, die) > x(&w, other) + 0.5, "passed through");
    let json = exact_game::json::to_string(&*w.get::<Collider>(die).unwrap()).unwrap();
    assert!(!json.contains("extra"), "{json}");
}

#[test]
fn extra_colliders_are_refused_without_a_body_and_as_sensors_or_meshes() {
    let mesh = Shape::Mesh {
        vertices: vec![Vec3::ZERO, Vec3::X, Vec3::Z],
        indices: vec![[0, 1, 2]],
    };
    let extra = |x: Collider| Collider {
        extra: vec![x],
        ..Collider::default()
    };
    for (body, collider) in [
        (false, extra(Collider::default())),
        (
            true,
            extra(Collider {
                sensor: true,
                ..Collider::default()
            }),
        ),
        (
            true,
            extra(Collider {
                shape: mesh,
                ..Collider::default()
            }),
        ),
        (true, extra(extra(Collider::default()))),
    ] {
        let mut w = world();
        let e = w.spawn((Transform::default(), collider));
        if body {
            w.insert(e, Body::default());
        }
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| physics::step(&mut w)));
        assert!(result.is_err());
    }
}
