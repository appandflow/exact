# Recipe: phone release builds

Run from the app directory. `--json` prints `path`, `target`, `trust` and (on Apple)
`signing` on stdout; failures print `code` and `error`. Build output goes to stderr.
It takes no `--run`, `--prebuild` or `--test`. Drive a development build; releases
omit the agent.

Every production bake with an update store needs `deploy.signing.keys` and either
`EXACT_UPDATE_RECEIPT` (an authenticated publisher receipt) or
`EXACT_UPDATE_GENESIS=1` (a new stream), including simulator and Android builds.
Store `"0"` embeds without updates, keys or receipts: set `deploy.store.ios: "0"`
(a first iOS build sets it only when `host.ios` is absent).

**iOS Simulator**, no installation or signing team:

```sh
bun exact.mjs ios --release --no-install --json
```

Omit `--no-install` to install; add `--sim <udid|name>` to select a simulator.

**Signed iPhone:** needs a phone this Mac knows, an Apple Development identity
with its private key, and a profile covering its UDID, bundle id and entitlements:

```sh
bun exact.mjs ios --device --phone <udid> --release --no-install --json
```

Exact chooses this Mac's profile and identity. To select them, prefix with
`EXACT_PROFILE=<file.mobileprovision> EXACT_IDENTITY=<certificate-sha1>`
(`security find-identity -v -p codesigning` lists identities).
`DEVELOPMENT_TEAM` alone does not select them. Installing needs it connected.

**No team or phone:** build an ad-hoc archive for a service that re-signs it:

```sh
bun exact.mjs ios --device --release --archive .exact/app.ipa --unsigned --json
```

Needs an AppIcon (`host.ios.icon`, or a declared square icon at least 512 px).
`--unsigned` requires `--archive`; no unsigned device `.app` build-only flag.

**Standalone Android APK:** set `deploy.store.linux: "0"` in app.json first.
Android uses the Linux bake; an absent Linux store defaults to updates, even in
a new app.

```sh
EXACT_UPDATE_TRUST=production bun exact.mjs android --release --json
```

It installs when an adb device is ready (`ANDROID_SERIAL` selects one); with none,
it only builds. If modules name Maven libraries, use `--write-locks` once and keep
the generated lock/verification files; subsequent builds verify them.

|Artifact (apps outside Exact)|Location|
|---|---|
|iOS `.app`|`target/clients/<source-key>/<bundle-id>/<destination>/<rust-target>/<composition>/production/standalone/<display name>.app`|
|Unsigned `.ipa`|the `--archive` path, relative to the working directory|
|Android APK|`target/android/app/outputs/apk/release/app-release.apk`|

Apple destinations: `ios-simulator` or `ios`; composition: `embedded` for store
`"0"`, otherwise `updating`. Read JSON's absolute `path`.

Needs pinned Bun/Rust (`bun <exact2>/scripts/exact.mjs setup --check`).
Apple: Mac, Xcode/iOS SDK 27.1+, target Rust std/Hermes; simulator runtime to run.
Android: SDK platform 37.2 (`ANDROID_HOME`), platform-tools, NDK, JDK 17+
(`JAVA_HOME`), `aarch64-linux-android` Rust std/Hermes. Install Xcode/SDKs before
`setup`, which installs supported missing dependencies.
