const nav = document.querySelector("nav.tabs");
if (nav && !document.getElementById("foundry-link")) {
  const link = document.createElement("a");
  link.id = "foundry-link"; link.textContent = "Foundry";
  link.href = "/foundry" + location.hash; link.target = "_blank"; link.rel = "noopener";
  nav.append(link);
}
