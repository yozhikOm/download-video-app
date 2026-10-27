# Design variants

`variants.html` holds clickable mockups of three looks for the main screen (issue #40). It is one self-contained page: open it in a browser, no build needed. Fonts come from Google Fonts and fall back to system fonts offline.

The mockups run on example data and use the texts from `src/strings.ts`. Paste a URL and press «Найти», choose a Preset, press «Скачать» and watch the Download go through the Queue. The controls above the window switch the example (a found Video, an empty window, a failed Probe), the window size (1000×700 or the minimum 760×560) and the theme.

1. **Родной Windows** — light, in the style of Windows 11. A horizontal Video card and the Queue as a dense list. The smallest change from today's layout.
2. **Две панели** — dark, across the whole window. Adding a Video on the left, the Queue always in view on the right with status counts.
3. **Один шаг** — light and large. A big field for the URL, three tiles for what to save, the Queue folded into a bar at the bottom.

Under the window, each variant lists its idea, what it makes easier and what it changes in the code.
