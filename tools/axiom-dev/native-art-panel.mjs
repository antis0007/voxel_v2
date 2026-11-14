import { el, button, panel } from "./inspector.mjs";

export function nativeArtPanel(api) {
  let nativeArtUrl;
  const nativeArt = el("div", {});
  const renderArt = button("Render compiled tool art", () => showNativeArt("/api/render-tool-art"),
    "Render the compiled tool sprites without opening a world or changing a save");
  const renderSaved = button("Render saved tool masks", () => showNativeArt("/api/render-saved-tool-art"),
    "Build the small native tool composer offline and render saved mask edits");
  async function showNativeArt(route) {
    renderArt.disabled = renderSaved.disabled = true;
    nativeArt.replaceChildren(el("p", { role: "status" }, "Rendering native tool art. First saved-mask preview builds the tool composer."));
    try {
      const result = await api(route, undefined, {});
      if (nativeArtUrl) URL.revokeObjectURL(nativeArtUrl);
      const bytes = Uint8Array.from(atob(result.png), character => character.charCodeAt(0));
      nativeArtUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      const image = el("img", { src: nativeArtUrl, alt: "Native tool appearances", style: "max-width:none;image-rendering:pixelated" });
      const scale = el("select", { "aria-label": "Native art scale", onchange: () => { image.style.width = result.width * Number(scale.value) + "px"; } },
        [1, 2, 4, 8].map(value => el("option", { value: String(value) }, value + "×")));
      scale.value = result.sourceSha256 ? "4" : "1";
      image.style.width = result.width * Number(scale.value) + "px";
      nativeArt.replaceChildren(el("p", {}, "Build: " + result.buildTime + " · " + result.width + " × " + result.height),
        el("p", { className: "muted" }, result.scope),
        el("label", {}, "Preview scale", scale), el("div", { style: "overflow:auto;max-height:65vh" }, image),
        el("a", { href: nativeArtUrl, download: "native-tool-art.png" }, "Download native sheet"),
        el("p", { className: "muted" }, "Evidence: " + result.output));
    } catch (error) { nativeArt.replaceChildren(el("p", { className: "error" }, error.message)); }
    finally { renderArt.disabled = renderSaved.disabled = false; }
  }

  return panel("Native art evidence", el("div", { className: "chips" }, renderArt, renderSaved), nativeArt);
}
