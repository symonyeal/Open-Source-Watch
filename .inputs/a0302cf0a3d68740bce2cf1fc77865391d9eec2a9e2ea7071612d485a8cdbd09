# Labelling guide: Russia/Ukraine event detail (phase 2)

Each input line is one report: `n` and `text` (English translation, at most 280
characters; may be cut off or
partly Ukrainian/Russian). Label only from the text. Do not use outside knowledge of what
happened, except to know what a named weapon or place is (for example that a Kh-101 is a
cruise missile or that Engels is a Russian airbase town).

Write one JSON object per input line, same order, with exactly these keys:

| key | values | rule |
| --- | --- | --- |
| `n` | the input `n` | |
| `event` | `true` / `false` | `true` if the text reports at least one specific violent or military incident that happened (a strike, attack, shelling, battle, shoot-down, interception, explosion, drone or missile launch at a place, casualties from an attack), including a summary of several such incidents ("Russian attacks kill 9 across Ukraine"). `false` for statements, opinion, analysis, plans, threats, warnings, diplomacy, sanctions, economic effects, aid, appeals, capability news, history, and alerts or threat notices with no incident ("ballistic threat", "UAVs heading west"). |
| `type` | `battles`, `explosions_remote_violence`, `violence_against_civilians`, `strategic_developments`, `protests`, `riots`, or `null` | ACLED event type of the main incident; `null` when `event` is false. Air, drone, missile and artillery strikes are `explosions_remote_violence`; ground fighting `battles`; shoot-downs and interceptions of drones or missiles `strategic_developments`. |
| `sub_event` | see list, or `null` | ACLED sub-event type of the main incident; `null` when `event` is false. |
| `weapon` | list drawn from `drone`, `missile`, `glide_bomb`, `artillery`; may be empty | Every weapon class the text says was used in the incidents it reports. `drone`: UAV, Shahed, Geran, FPV, "kamikaze drone", Lancet. `missile`: any missile (ballistic, cruise, Iskander, Kh-101, Kalibr, Kinzhal, S-300, Storm Shadow, ATACMS, Neptune). `glide_bomb`: guided aerial bomb, KAB, glide bomb, UMPK, FAB with a guidance kit. `artillery`: shelling, artillery, MLRS, Grad, Uragan, mortar, tank fire. Weapons only intercepted still count. Empty when no weapon is named. |
| `missile_type` | string or `null` | The most specific missile named ("Iskander-M", "Kh-101", "Kalibr", "Kinzhal", "S-300", "Storm Shadow", "ATACMS", "Neptune", "Oreshnik"); else `"ballistic"` or `"cruise"` if only that is said; else `null`. `null` if no missile. |
| `target` | `energy`, `military`, `civilian`, `infrastructure`, or `null` | Class of what was hit or aimed at in the main incident. `energy`: refinery, oil depot, oil terminal or pumping station, power plant, substation, power lines, gas facilities, "energy infrastructure". `military`: airfield, military unit, troops, equipment, ammunition depot, defence plant, command post, air defence system. `infrastructure`: bridge, port, railway, dam, roads, telecoms, water supply, non-defence industry. `civilian`: homes, residential buildings, people, shops, schools, hospitals, cars, public transport. `null` if no target is said or `event` is false. If several, the first named. |
| `killed` | integer or `null` | People killed in the incidents reported, as the text states the total ("13 killed", "at least 9" gives 9, "a man died" gives 1). `null` if not stated. Do not count attackers' drones or missiles. |
| `injured` | integer or `null` | People injured, same rule. |
| `actor1` | `Russia`, `Ukraine`, `other`, or `null` | The side that carried out the main incident (for a shoot-down: the side whose weapon was shot down). `null` if not said and not clear from the text. |
| `actor2` | `Russia`, `Ukraine`, `other`, or `null` | The side attacked (where the hit fell; for a shoot-down: the side that shot it down). `null` if unclear. |

ACLED sub-event types (ACLED Codebook 2023 §2):

- battles: `Armed clash`, `Government regains territory`, `Non-state actor overtakes territory`
- explosions_remote_violence: `Air/drone strike`, `Shelling/artillery/missile attack`, `Remote explosive/landmine/IED`, `Suicide bomb`, `Grenade`, `Chemical weapon`
- violence_against_civilians: `Attack`, `Abduction/forced disappearance`, `Sexual violence`
- strategic_developments: `Disrupted weapons use` (interceptions, shoot-downs, defused explosives), `Agreement`, `Arrests`, `Change to group/activity`, `Headquarters or base established`, `Looting/property destruction`, `Non-violent transfer of territory`, `Other`
- protests: `Peaceful protest`, `Protest with intervention`, `Excessive force against protesters`
- riots: `Violent demonstration`, `Mob violence`

Sub-event rules: a drone strike or an air-dropped bomb (including glide bombs) is
`Air/drone strike`; a missile, artillery, MLRS or mortar attack is
`Shelling/artillery/missile attack`. For a combined missile and drone attack, use the
first-named weapon (this sample's convention).

Write the output as JSON lines, nothing else.
