import("./app.mjs").catch(error => {
  const status = document.querySelector("footer [role=status]");
  if (status) status.textContent = "Dev Panel could not start · " + error.message;
  console.error(error);
});
