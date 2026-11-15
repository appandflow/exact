# The widget gallery shows an empty timeline for a widget that reads storage

**Status:** Closed
**Resolution:** Every gallery read of a refreshed preview says the refresh read no saved data and sent no requests (storage), the gallery's caption says so, and the widget docs say how to check such a widget; reading the app's shared data in the gallery is not done.
**Systems:** widget gallery (`WidgetGallery.swift`), scripts/agent-widget.mjs, docs
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks' mileage widget reads saved totals and a photo from the app's shared data. In the development gallery it shows an empty timeline, with white margins around the coloured widget, though the real widget on the Home Screen shows the data. The lane spent its time checking the saved store before finding that the gallery reads none.

## Cause

By design, the gallery's in-process stand-in makes no storage reads or requests.

## Fix

Let the gallery read the app's saved shared data when asked (a flag, or by default under the agent), and say in its output when a timeline was built without storage. Put that limit beside the gallery commands in the widget docs.

## Resolution

The minimum fix. The gallery still refreshes on an empty, disposable store and executes no requests. Every agent read (`tree`, `state`, `screenshot`) of a preview a refresh made now carries `storage`, a sentence saying the refresh read none of the app's saved data and sent no requests, while the extension's refresh on a Home Screen reads both; a read with `--props` ran no refresh and carries none. Each gallery cell's caption ends "no saved data or requests". `docs/reference.md`, beside the gallery's agent addresses, says the same and points to `--props` or a Home Screen for such a widget.

Reading the app's saved data in the gallery was not done. It would mean running the extension's refresh path (`Host::entry_timeline`: the app's directories, the I/O endowment and an executor) inside the running app, whose gallery refresh deliberately uses a separate data seam so as not to reset the app's picked-file roots (`Bridge::refresh_entry`). That is more than the gallery is now.

## Verification, 2026-11-10

`bun host/apple/build.mjs --test --only WidgetGalleryTests` (macOS) passes, 9 tests. `testReadsUseTheRefreshAtTheAppClockAndTheCurrentGeneration` now checks that `state` and `tree` of Caltrain's refreshed `next-trains` carry the note, and that a read with supplied props does not.
