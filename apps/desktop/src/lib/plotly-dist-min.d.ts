// `plotly.js-dist-min` ships no types of its own; it's the same API surface
// as `plotly.js` (just pre-bundled/minified), so reuse those types.
declare module "plotly.js-dist-min" {
  import * as Plotly from "plotly.js";
  export = Plotly;
}
