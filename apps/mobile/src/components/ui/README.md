# gluestack-ui components

Copied, not depended on: gluestack-ui v5 is shadcn-shaped, so each component
lives in this repo and is ours to edit.

Source: the `main` branch of `github.com/gluestack/gluestack-ui`, under
`apps/starter-kit-expo-uniwind/components/ui/<name>/`. That is the Uniwind
variant — the same components as the NativeWind v5 starter kit, written against
the styling engine this app actually uses (see `apps/mobile/metro.config.js`
for why it is Uniwind and not NativeWind).

To add one:

1. Copy `<name>/` from that directory, dropping `index.web.tsx` and `script.ts`
   — this app has no web target.
2. Install whatever it imports. `heading` needs `@expo/html-elements` and
   `icon`/`badge` need `react-native-svg`; neither is installed, so prefer
   `text` with a size class and `lucide-react-native` (which also wants
   `react-native-svg`) only once an icon is actually on a screen.
3. The colour classes are the shadcn slots (`bg-primary`, `border-border`, …).
   They are defined in `apps/mobile/global.css` with the desktop's values; a
   component using a slot that is not there renders unstyled rather than
   failing, so check the slot exists.

Only the components a screen renders live here. `pnpm run check:dead-code`
fails on one nothing imports, which is the intended pressure: copy on demand.
