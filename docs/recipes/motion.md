# Recipe: motion

Use `transition` for a changing value, `keyframes` and `animation` for entrances or loops,
`-exact-exit-animation` for removal, `-exact-layout-transition` for layout.

```contract
keyframes rise
  from opacity=0 translate="0px 8px"
keyframes leave
  to opacity=0 scale=0.96 animation-timing-function="ease-in"
keyframes breathe
  50% opacity=0.4

shape Media
  prefersReducedMotion: bool

component App
  resource media = exactViewport() as shape Media
  derive still = media.prefersReducedMotion
  state items = ["Kick", "Snare", "Hat"]
  state open = false
  action toggle
    open = not open
  action remove(name: string)
    items = items.filter(i => i != name)
  view
    column padding=16 gap=8
      button press=toggle testId="toggle"
        text "Details"
      text "Saving…" animation=(still ? "none" : "breathe 1.6s ease-in-out infinite")
      column height=(open ? "auto" : "0px") overflow="hidden" inert=(not open)
        -exact-layout-transition=(still ? "none" : "320ms ease")
        text "Shown" opacity=(open ? 1 : 0) transition=(still ? "none" : "opacity 200ms ease")
      for name, i in items key=name
        row gap=8 animation=(still ? "none" : `rise 300ms ${i * 60}ms both`)
          -exact-layout-transition=(still ? "none" : "320ms ease")
          -exact-exit-animation=(still ? "none" : "leave 200ms ease-in both")
          text name flex=1
          button press=remove(name) testId=`remove-${name}`
            text "Remove"
```

- **What animates:** translate, scale, rotate, opacity, colours, `box-shadow`, SVG geometry,
  a numeric `height`, `font-weight`, `font-stretch`, `font-variation-settings`; other layout
  properties are refused. A native `button` animates only opacity and transforms.
- **`-exact-spring(stiffness, damping, mass)`** is a transition's timing, with no duration;
  an `animation` refuses it.
- **Keyframes** take literals or the app's own `fn` on constants.
- **Exit:** plays when an `if` arm or `for` row leaves, and must end. The node leaves layout
  at once, inert; others move by their layout transition. Not in a screen being popped, nor
  on Linux.
- **Layout transition:** a transition without a property. It moves the box and resizes its
  surface; children jump to their place, unscaled. `sharedElement=` flies between boxes on
  iOS, macOS and the web, not Linux.
- **Expand and collapse:** keep the content mounted (`Shown`) in a clipped wrapper whose
  `height` moves between `"0px"` and `"auto"`. An `if` row takes its whole box at once, and
  the rows below slide over it.
- **JS target:** a bound exit or layout value must be one of the binding's literals.
- **Stagger** with delays (`"a 200ms both, b 300ms 200ms both"`). A state change when a
  motion ends is a gated task's `after(ms, action)`.
- **Reduced motion:** transitions and keyframes run unless the app gates them on
  `exactViewport()`, as above; drive it with `prefer prefers-reduced-motion reduce`.
- **Check it:** `clock settle` ends the motion; `screenshot .exact/m.png over 600 every 50`
  films it.
