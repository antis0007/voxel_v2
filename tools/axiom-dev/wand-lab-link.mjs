const nav = document.querySelector("nav.tabs");
if (nav && !document.getElementById("wand-lab-link")) {
  const link = document.createElement("a");
  link.id = "wand-lab-link"; link.textContent = "Wand Lab";
  link.href = "/wand-lab" + location.hash; link.target = "_blank"; link.rel = "noopener";
  nav.append(link);
}
