# Regional Civilization release candidate

Generator 322 is the default for new Hills, Ridged and Planet worlds. Older
saves use their recorded generator version.

- The loading map shows living settlements, surveyed roads and Anchor city ruins.
- Generated Hills and Ridged communities run the same stock, work and migration
  systems as Planet communities.
- A resident can request bread, wheat or planks when their settlement falls below
  its stock target. Deliver the requested item beside the settlement stockpile.
  Residents draw on the same stockpile for work and meals.

## Playtest

Start an isolated world with the Axiom dev panel. Use seed 424242 for comparison.
Observe the loading map and enter the world. Locate an active settlement, speak to
a resident beside its stockpile, and fulfil a supply request. Compare player
inventory and community stock before and after, then check a later production
or construction update.

Use `node tools/axiom-dev/civilization-cli.mjs live` to inspect the native
community ledger. Use the `review` command for repeatable regional maps.

Release acceptance requires a full survival run through discovery, delivery and
a visible settlement response, plus a save and reopen. A clean build or a
settlement scheduler starting up is a smoke check.

## Native validation

The local build reached gameplay on Hills, Ridged and Planet with generator 322.
The Hills loading screen displayed the regional map. The Hills Survival run
admitted 11 residents across three settlements, and one stockpile opened with
eight bread and 16 planks. Ridged admitted eight residents across two settlements.
The Planet run reached playable terrain but had admitted no communities at its
last inspection. The supply delivery and save and reopen checks remain pending.
