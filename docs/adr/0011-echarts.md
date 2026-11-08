# 0011. Apache ECharts as the single chart library

- **Status:** Accepted (adopted in M3)
- **Date:** 2026-11-08

## Context

The product needs line, stacked area, bar, heatmap, calendar, treemap, scatter, waterfall and sankey views that perform with large data, in one consistent visual system.

## Decision

- **Library.** Use Apache ECharts 6.1+ (Apache-2.0), imported tree-shaken from `echarts/core`, behind shared chart primitives in `shared/charts`.
- **Tooltips.** Always use custom formatters that escape their content (advisory GHSA-fgmj-fm8m-jvvx).

## Alternatives considered

- **Recharts:** lacks calendar and sankey at the needed quality.
- **Nivo:** stale, and SVG-based.
- **visx / D3:** everything would have to be built by hand.
- **uPlot:** time series only.

## Consequences

- One theme, generated from the design tokens.
- No second chart library unless an ADR justifies it.

## Revisit when

ECharts cannot express a required view, or bundle or parse cost fails the performance budgets on WebKitGTK.
