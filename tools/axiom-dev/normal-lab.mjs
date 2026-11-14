import { el, button } from "./inspector.mjs";

export function openNormalLab(canvas, name) {
  const frame = el("iframe", {
    title: "Pixel Art Lighter normal map laboratory",
    sandbox: "allow-scripts allow-downloads",
    style: "width:100%;height:72vh;border:0",
  });
  const status = el("p", { role: "status" }, "Preparing painted sprite.");
  const dialog = el("dialog", {
    style: "width: min(1500px, 96vw);max-width:96vw",
    "aria-label": "Normal map laboratory",
    onclose: () => dialog.remove(),
  }, el("header", {}, el("h2", {}, "Normal map laboratory"),
    button("Close", () => dialog.close())), status,
    el("p", {}, "Review candidates only. Axiom does not load authored normal textures yet. Export Normal to retain a candidate; Light Mask is not an Axiom material mask."), frame);
  frame.addEventListener("load", () => canvas.toBlob(blob => {
    if (!blob || !dialog.isConnected) return;
    frame.contentWindow.postMessage({ type: "axiom-normal-albedo", blob, name: name + ".png" }, "*");
    status.textContent = "Painted sprite sent. Generate Normal, adjust surface hints, then move the light to check relief.";
  }, "image/png"));
  frame.src = "/normal-lab.html";
  document.body.append(dialog);
  dialog.showModal();
}
