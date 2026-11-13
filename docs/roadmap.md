# Roadmap: v0.0 → v1.0 and the long-term track

| | |
|---|---|
| **Document version** | 2.1 (2026-11-12) |
| **Owner** | Bl4202 |
| **Status** | Accepted (2026-11-12, [ADR-50](adr/0050-entering-m2.md)). The owner accepted the content schedule (section 3) and every reconciliation row (section 19); the GDD text the rows name is amended in GDD 0.2.5 |
| **Sources** | [GDD.md](GDD.md) (the specification), [GDD_Outline.md](GDD_Outline.md) (the original intent), the ADRs in [adr/](adr/), [open-questions.md](open-questions.md), the exit records in [exit/](exit/) |
| **Supersedes** | Roadmap 1.x (the short version-by-version list, last changed 2026-11-09) |

The GDD says *what* the game is. This roadmap says *when* each part of it is built, *which content* ships with it, and *how* each step is proved done. It expands GDD §66 (Development Roadmap), §67 (Prototype Requirements) and §68 (Design Priorities) and it resolves the places where those chapters, the chapter tags and the outline disagree (section 19).

**References.** "§n" is a GDD section; "outline §n" is a section of the outline; "section n" is a section of this document. ADR, OQ, AC, NN, PS, ET and G identifiers are the GDD's (§0, Appendix I.5).

---

# TABLE OF CONTENTS

1. How to read this roadmap
2. Overview
3. Content plan
4. System staging matrix
5. v0.0 Foundations
6. v0.1 Flight Prototype
7. v0.2 Geographic Sandbox
8. v0.3 Combat
9. v0.4 Procedural Missions
10. v0.5 Multiplayer
11. v0.6 Campaign
12. v0.7 Content & Polish
13. v0.8 Beta Hardening
14. v0.9 Release Candidate
15. v1.0 Release
16. Long-term track (M7)
17. Cross-cutting tracks
18. Decisions due, cut lines and risks
19. Reconciliation with the GDD and the outline
20. Change log

---

# 1. HOW TO READ THIS ROADMAP

## 1.1 Authority and precedence

1. **The outline** (outline §1–§153) is the original intent. **The GDD** turns it into a specification. **This roadmap** schedules the specification.
2. **Owner decisions and accepted ADRs** override the GDD and this roadmap for the scope they name (for example ADR-40, the custom renderer; ADR-45 decisions D1–D3 for v0.1d).
3. Where the GDD disagrees with itself (most often a chapter's `P · M` tag against §61.5, §66.3 or §67) or with the outline, this roadmap picks one reading, records it in section 19 with the reason, and lists the GDD text that should be amended through the GDD's change process (Appendix I.4). The readings are chosen in this order:
   1. what the version's playable build or exit test needs (for example, the two-location test of M2 needs basic navigation);
   2. the priority rules of §68.2 (a lower-priority system waits until something needs it);
   3. the outline;
   4. the milestone plan of §66.3 and §67;
   5. the chapter's own `P · M` tag;
   6. the seam table of §61.5.
4. Detailed work packages live in each sub-milestone's plan and gates ADR (for example ADR-45 and the v0.1d plan), not here. This roadmap fixes scope, content and exit criteria; the plan fixes how.

## 1.2 Versions, milestones and builds

| Term | Meaning | Example |
|---|---|---|
| **Version** | One GDD milestone (M0–M6), or a release-hardening stage after M6 | v0.3 = M3 Combat |
| **Sub-milestone** | A slice of a version that ends with something runnable on the staging URL and a testable exit check | v0.3b = First Playable Build |
| **Work package** | A step inside a sub-milestone's plan | v0.1d WP4 |
| **Playable build** | A scripted acceptance run from §67 | First Playable (§67.1) |
| **Gate** | A quality gate from §66.4, fixed with thresholds *before* the work starts and never weakened (`AC-66.4`) | G2 fun gate |
| **Release train** | A post-1.0 content release (§66.7) | "Naval & subs" |

Every sub-milestone has, before its code starts: a gates ADR (ADR-33, ADR-39 and ADR-45 are the pattern), an exit checklist skeleton in `docs/exit/`, and a branch. It closes with the exit record, the merge(s) and an annotated tag.

## 1.3 Status markers

| Marker | Meaning |
|---|---|
| ✅ | Done and tagged |
| ▶ | In progress |
| ○ | Planned |
| ✂ | Cut candidate (section 18.3) |

## 1.4 Durations and the team

Durations are the GDD's planning estimates for a core team of 8–12 (§66). The project is solo (OQ-02, provisional), so every sub-milestone so far has been larger than its slot, and work is measured in engineer-weeks:

| Sub-milestone | GDD slot | Scope estimate | Gates fixed | Closed |
|---|---|---|---|---|
| v0.1a | 5 weeks | — | 2026-10-14 (ADR-31) | ✅ tagged `v0.1a` |
| v0.1b | 4 weeks | ≈ 13 engineer-weeks | 2026-10-22 (ADR-32, ADR-33) | ✅ tagged `v0.1b` (2026-10-28) |
| v0.1c | 4 weeks | ≈ 14.5 engineer-weeks | 2026-11-03 (ADR-39) | ✅ tagged `v0.1c` (2026-11-08) |
| v0.1d | 3 weeks | ≈ 13.6 engineer-weeks | 2026-11-09 (ADR-45) | ✅ tagged `v0.1d` (2026-11-11) |

The month figures below are kept as relative sizes. They will be replaced once OQ-02 (team size and release scale) is decided.

## 1.5 Rules carried from the GDD

1. **Feel first** (§66.1.1). No content breadth work starts until the flight-feel gate (G1) passes.
2. **Vertical slices** (§66.1.2). Every sub-milestone exercises the whole pipeline (data → sim → render → audio → UI) at its scope.
3. **Stubs for the future** (§66.1.3, §61.5). Anything deferred has an interface and a stub; replacing the stub does not change its callers (`AC-66.2`).
4. **Cut by priority, not by area** (§66.1.4, §68.1): P1 flight physics → P2 geographic sandbox → P3 combat → P4 AI → P5 procedural missions → P6 large battles → P7 LAN/VLAN → P8 1v1 → P9 campaign → P10 advanced systems.
5. **Always shippable** (§66.1.5). `main` deploys to staging on every merge; unfinished features sit behind flags.
6. **Measure continuously** (§66.1.6). Performance (§60.9), conformance (§11.19), determinism (ADR-10) and accessibility (§57.8) checks run in CI from the version where they first apply.
7. **Data models reserved from M1** (§66.2): ids, `PilotId`, `AirframeId`, `PersistenceSink`, `IForceProvider`, `IAirframeSource`, `StrategicContext`, the `MissionEvent` stream and the `MissionResult` record exist as real types long before they are filled.
8. **Regression rule** (§68.2.3). A change that degrades a higher-priority metric (flight latency, G1, conformance) to help a lower-priority feature is rejected unless the owner waives it with a recorded rationale and an expiry.

## 1.6 Human-panel gates in a solo project

Several GDD gates need people the project does not have. v0.1d set the precedent (ADR-45, decision D1): automated proxies gate the tag, the owner's own runs are recorded and do not block, and the panel stays open as an open question (OQ-11). The same choice has to be made, version by version, for every gate below. **Decided (2026-11-12, OQ-12, [ADR-50](adr/0050-entering-m2.md)):** the owner chose D1's rule for every panel. Each gates ADR names the automated proxy that decides each panel gate; the owner's own runs are recorded and do not block; a real panel stays possible and is recorded beside the proxy.

| Gate or test | Version | What the GDD asks for | Automated proxy available | Note |
|---|---|---|---|---|
| G1(a) flight feel | v0.1d | ≥ 12 blind testers, A/B against two references (§66.4) | Conformance, the NN-2 audit, the bot playtester (ADR-45) | Open as OQ-11 |
| `AC-4.5` precursor | v0.1d | ≥ 80 % of external playtesters take off, shoot a bandit and land in 10 minutes | The bot playtester (BP1) | Decided (D1) |
| **G2 fun gate** | v0.3b | ≥ 20 external testers; ≥ 70 % would play another sortie | The 11-step script runs automated (`AC-67.1`), but **nothing automates "fun"** | The roadmap recommended a real panel; the owner chose the automated run plus the owner's own sorties (OQ-12). The owner's sorties are the only signal of "fun"; risk R-05 is accepted |
| §67.2 run | v0.3d | ≥ 8 of 10 external testers through step 6 | The automated run through step 9 (`AC-67.2`) | |
| G3 replayability | v0.4b | Paired sessions; ≥ 80 % rate them "substantially different" | `AC-3.4` (mean DNA distance ≥ 0.45) | |
| Usability | v0.4c | SUS ≥ 75 | First-flight completion (`AC-53.7`) | |
| ET-1 / ET-3 / ET-2 | v0.4c / v0.5e / v0.6d | Beat-sheet sessions (§70) | Their quantitative parts | |
| G5 network | v0.5e | Invite flow on 20 home networks | Impairment matrix, 16-client soak | |
| G6 campaign loop | v0.6d | ≥ 60 % of testers continue past their third operation | The 60-run campaign soak | |
| G7 accessibility | v0.5e, v0.7b | External accessibility testers (§57.8) | §57.8 automated checks | §71.6 requires the external review for 1.0 |

---

# 2. OVERVIEW

## 2.1 Versions

| Version | Name | GDD milestone | Outline phase | Priorities | Sub-milestones | Est. duration | Status | Playable result |
|---|---|---|---|---|---|---|---|---|
| **v0.0** | Foundations | M0 | — (§125) | infrastructure | a, b | 2 mo | a done, b done (the owner waived laptop runs C and D on 2026-10-16); not tagged | Tech demo: render + sim + input + CI loop |
| **v0.1** | Flight Prototype | M1 | §126 | P1 | a, b, c, d | 4 mo | a ✅ b ✅ c ✅ d ✅ (M1 complete on bot evidence; G1(a) open, OQ-11) | One aircraft that flies well, a gun, an IR missile, one bandit |
| **v0.2** | Geographic Sandbox | M2 | §127 | P2 | a, b, c, d | 5 mo | ▶ v0.2a in progress (gates fixed 2026-11-12, ADR-51) | Pick two places on Earth and fly between them |
| **v0.3** | Combat | M3 | §128, §132, §133 | P3, P4 | a, b, c, d | 6 mo | ○ | **First Playable** (v0.3b), **Second Playable** (v0.3d) |
| **v0.4** | Procedural Missions | M4 | §129 | P5 (P4) | a, b, c | 5 mo | ○ | Generated, escalating missions anywhere; 14 mission types |
| **v0.5** | Multiplayer | M5 | §130, §134 | P6, P7, P8 | a, b, c, d, e | 5 mo | ○ | **Third Playable**; public beta of the sandbox product |
| **v0.6** | Campaign | M6 | §131, §135 | P9 | a, b, c, d | 6 mo | ○ | **Campaign Build** |
| **v0.7** | Content & Polish | M6 exit, G4, G7, G8 | §145 (start) | P10 | a, b | 3 mo | ○ | Release content complete and polished |
| **v0.8** | Beta Hardening | G5/G6 re-runs | — | all | a, b | 2 mo | ○ | The full game in the public beta |
| **v0.9** | Release Candidate | G9 preparation | — | all | a | 1 mo | ○ | Feature freeze, bug-fix only |
| **v1.0** | Release | G9 | §151–§153 | — | — | — | ○ | Stable channel |
| **M7** | Long-term content | M7 | §145 | P10 | release trains | ongoing | ○ | Content releases, community hub, advanced systems |

## 2.2 Playable builds and release channels

The channel plan follows §66.8: `alpha` from M3, public opt-in `beta` from M5, `stable` at 1.0.

| Build | Version | Acceptance run | Gate | Channel opened or used |
|---|---|---|---|---|
| Tech demo | v0.0b | — | — | `nightly` (staging, every merge) |
| Flight prototype | v0.1d | The bot sortie under every assist (ADR-45) | G1 | `nightly` |
| **First Playable (FP1)** | v0.3b | §67.1, eleven steps | **G2** | **`alpha` opens** (internal and invited, weekly) |
| **Second Playable (FP2)** | v0.3d | §67.2, nine steps | — | `alpha` |
| Generated missions | v0.4c | ET-1 (§70.2) | G3, G7 baseline, G4 audio | `alpha` |
| **Third Playable (FP3)** | v0.5e | §67.3 | **G5**, G7 complete | **`beta` opens** (public, opt-in): Sandbox, 1v1 and co-op, the "first shippable product" of §1.4 |
| **Campaign Build** | v0.6d | §67.4 | **G6** | `beta` |
| Release | v1.0 | §71.6 checklist | **G9** | **`stable`** |

## 2.3 Game modes by version (§6.1)

| Mode | First playable | Complete | Notes |
|---|---|---|---|
| Free Flight | v0.1c (test arenas) | v0.2d (anywhere on Earth) | |
| Training | v0.4c (First Flight + 8 missions, §53.11) | v0.4c | Mission 9, carrier ops, is post-launch (M7) |
| Sandbox Skirmish | v0.3b (one dogfight template) | v0.4c (full generator and configuration) | |
| 1v1 | v0.4c (against the AI on one machine, §31.8 arenas) | v0.5c (two humans, rule sets, spectating) | |
| LAN/VLAN Co-op | v0.5c | v0.5e | 2–16 players, ≤ 8 recommended in a browser host (§33.9) |
| Roguelite Conquest Campaign | v0.6a | v0.6d | Co-op campaign (§32.11) is post-launch (M7) |
| Replay Viewer | v0.4c (own sorties, SSR) | v0.5e (omniscient replays, multiplayer rules) | |

## 2.4 Timeline (GDD estimates)

| Span | Months from the start of M0 |
|---|---|
| M0 Foundations | 0–2 |
| M1 Flight Prototype | 2–6 |
| M2 Geographic Sandbox | 6–11 |
| M3 Combat (FP1 at about month 14) | 11–17 |
| M4 Procedural Missions | 17–22 |
| M5 Multiplayer (public beta at the end) | 22–27 |
| M6 Campaign | 27–33 |
| v0.7–v1.0 | 33–39 |

## 2.5 Dependency order (critical path)

Foundations (v0.0) → flight model (v0.1) → terrain and streaming (v0.2) → weapons and sensors (v0.3a) → AI (v0.3c) → mission system and generator (v0.4) → networking (v0.5) → campaign (v0.6) → polish and release (v0.7–v1.0).

Forces at scale, the naval layer and carriers (v0.5d) follow networking because M5's scope and the Third Playable include them (§66.3, §67.3, outline §134; row R1 in section 19). Art, audio and data tracks run in parallel and are off the critical path unless noted (section 3).

---

# 3. CONTENT PLAN

This section is the schedule for everything that is data, art or audio rather than engine: aircraft, weapons, units, ships, factions, mission types, places and presentation. The launch counts come from Appendix D.1 and §71.6; the per-version placement is this roadmap's, and each placement names the reason (usually the playable build or mission type that needs it). All names are fictional working names pending the naming review (ADR-11, G8).

## 3.1 Content counts by version (cumulative)

| Content | v0.1 | v0.2 | v0.3 | v0.4 | v0.5 | v0.6 | v0.7 | 1.0 target (source) |
|---|---|---|---|---|---|---|---|---|
| Combat aircraft, player-flyable | 1 | 1 | 5 | 7 | 12 | 12 | 12 | 12 (§13.4) |
| Combat aircraft present only as AI | — | — | 1 (Argus) | 1 | — | — | — | — |
| Support aircraft (AI; Scout also flyable) | — | — | 1 | 3 | 3 | 3 | 3 | 3 (D.2) |
| Weapons | 2 | 2 | 13 | 20 | 26 | 26 | 26 | 26 (§15.12) |
| Non-weapon store types | — | — | 2 | 4 | 6 | 6 | 6 | 6 (§15.12) |
| Ground unit types (D.3 ids) | — | — | 16 + installations | 25 + infrastructure | all | all | all | ≈ 30 types in 11 classes (§21.2) |
| Ship classes (D.4) | — | — | 2 (static targets) | 2 | 6 | 6 | 7 | 7 of 8 at 1.0; the eighth, the submarine, is M7 (R13 in section 19) |
| Factions | — | — | 2 | 4 | 4 | 4 | 4 | 4 (§37.3) |
| Mission types | — | — | 4 (hand-authored or early templates) | 14 | 16 | 16 | 16 | 16 (§27) |
| Twists | — | — | — | 17 | 17 | 17 | 17 | 17 (§69.4) |
| Campaign event deck | — | — | — | — | — | ≈ 24 | ≈ 24+ | ≈ 24 (§40.5) |
| Training missions | — | — | — | First Flight + 8 | 8 | 8 | 8 | 8, carrier ops post-launch (§53.11) |
| Pre-packaged regions | — | 6 | 6 | 10 | ≥ 20 | ≥ 20 | release set | ≥ 20 + worldwide streaming (§71.6) |
| Worldwide streaming | — | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Hero-polished airbases | 1 test airfield | — | 2 | 5 | 10 | 15 | ≥ 30 | ≥ 30 (§71.6) |
| Curated 1v1 arenas | 2 test arenas | — | — | 10 | 10 | 10 | 10 | 10 (§31.8) |
| Weather presets | static clear | 3 | 3 | 8 | 8 | 8 | 9 | 9 (§24.3) |
| AI skill levels | 3 | 3 | 5 | 5 | 5 | 5 | 5 | 5 (§19.6) |
| Callsign words | — | — | 120 | 120 | 120 | 120 | 120 | 120 (D.6) |
| UI languages | en | en | en | en | en + 8 | en + 8 | final list | §53.10 |

## 3.2 Aircraft (§13.4, Appendix D.2)

| # | Aircraft | Category | Era | Flyable from | AI from | Why at that version | Art (section 3.10) |
|---|---|---|---|---|---|---|---|
| 1 | **Kestrel** | Light Fighter | G4 | ✅ v0.1a | ✅ v0.1d (the bandit) | Reference aircraft (Appendix C) | Procedural low-poly ✅ v0.1c; hero model v0.2c |
| 2 | **Peregrine** | Multirole Fighter | G4.5 | v0.3d | v0.3d | §67.2 step 2 ("Peregrine with 4 × MRM + 2 × WVR") | Player-grade, v0.3 |
| 3 | **Swift** | Interceptor | G4 | v0.3d | v0.3d | §67.2 roster; the raid interceptor | Player-grade, v0.3 |
| 4 | **Bittern** | Strike Fighter | G4 | v0.3d | v0.3d | §67.2 roster; the strike in step 5 | Player-grade, v0.3 |
| 5 | **Badger** | Close Air Support | G3 | v0.3d | v0.3d | §67.2 roster; first G3 airframe; GH-30 | Player-grade, v0.3 |
| 6 | **Condor** | Heavy Fighter (pilot + WSO) | G4 | v0.4b (pilot seat) | v0.4b | Meridian Pact's heavy fighter for the generator; the WSO seat waits for multi-crew (v0.5c) | Player-grade, v0.4 |
| 7 | **Albatross** | Bomber (crew 3) | G3 | v0.4b | v0.4b | The Bombing mission type and the bomber role | Player-grade, v0.4 |
| 8 | **Argus** | AWACS (5+ seats) | G4 | v0.5c (ABM console) | v0.3d | AI AWACS for §67.2 step 4; human support role in co-op (§32.4) | AI-grade v0.3; stations v0.5 |
| 9 | **Magpie** | EW Aircraft (pilot + EWO) | G4 | v0.5c | v0.5c | The EW role and its console (§17.8, §32.4) | Player-grade, v0.5 |
| 10 | **Gannet** | Carrier Aircraft | G4 | v0.5d | v0.5d | Needs the carriers of v0.5d (catapult, arrester hook) | Player-grade, v0.5 |
| 11 | **Wraith** | Stealth Aircraft | G5 | v0.5e ✂ | v0.5e | G5 era for the beta; Pike-9 | Player-grade, v0.5 |
| 12 | **Chimera** | Experimental | G5 | v0.5e ✂ | v0.5e | Thrust vectoring needs FCS work (§11.11) | Player-grade, v0.5 |
| — | **Stratus** | Tanker | G4 | (tanker controls, v0.5c) | v0.3d | Aerial refuelling (§49.5) in the §67.2 build | AI-grade, v0.3 |
| — | **Hauler** | Transport | — | — | v0.4b | Convoy and logistics targets; civilian traffic (§23) | AI-grade, v0.4 |
| — | **Scout** | UAV | — | v0.4b (remote) | v0.4b | The Reconnaissance mission type and the recon role | AI-grade, v0.4 |

Era variants: the **Kestrel G3 variant** (Appendix D.7) arrives at v0.4b with the generator's *Aircraft Era* filter (§13.6). Every new airframe passes its own Performance Conformance Test (§11.19) before it becomes flyable; the roster values other than the Kestrel's are unvalidated design targets (Appendix I.6). **Cut line:** at least 8 combat aircraft at the beta (§66.6.2); Wraith and Chimera are the first to move to v0.7a.

## 3.3 Weapons and stores (§15.12)

| ID | Name | Class | Era | Version | Needed by |
|---|---|---|---|---|---|
| `gs27` | GS-27 | Revolver gun, 27 mm | G4 | ✅ v0.1d | The Kestrel's gun (ADR-46) |
| `dart1` | Dart-1 | SRM, rear-aspect IR | G3 | ✅ v0.1d | The v0.1d missile (simple proportional navigation) |
| `lance4` | Lance-4 | MRM, active radar | G4 | v0.3a | FP1's radar missile (§67.1 step 9) |
| `lance2` | Lance-2 | MRM, semi-active radar | G3/G4 | v0.3a | The SARH illumination model; the G3 era |
| `dart4` | Dart-4 | SRM, all-aspect imaging IR, HOBS | G4.5 | v0.3a | The Peregrine's WVR missile (§67.2) |
| `gr20` | GR-20 | Rotary gun, 20 mm | G3+ | v0.3d | The second gun class for the five-aircraft roster |
| `gh30` | GH-30 | Heavy rotary gun, 30 mm | G3+ | v0.3d | The Badger (CAS-class internal) |
| `b250` | B-250 | General-purpose bomb | G3+ | v0.3d | Bombs in §67.2 |
| `b500` | B-500 | General-purpose bomb | G3+ | v0.3d | Bombs in §67.2 |
| `rkt19` | RKT-19 | 19-tube rocket pod | G3+ | v0.3d | Rockets in §67.2 |
| `gb250l` | GB-250L | Laser-guided bomb | G4 | v0.3d | Precision strike with the targeting pod |
| `talon1` | Talon-1 | AGM, EO/IR imaging | G4 | v0.3d | Stand-off strike |
| `hunter2` | Hunter-2 | Anti-radiation missile | G4 | v0.3d | §67.2 step 6 (kill a SAM site) |
| `gs30` | GS-30 | Gun, 30 mm | G3+ | v0.4b | Condor and the G3 variants |
| `b900` | B-900 | Heavy bomb | G3+ | v0.4b | The Albatross; Bombing |
| `cb300` | CB-300 | Cluster bomb | G3+ | v0.4b | Area targets; Convoy Attack |
| `rp1` | RP-1 | Runway penetrator | G4 | v0.4b | Airbase strikes (Bombing, Dynamic Frontline) |
| `gb500g` | GB-500G | GPS/INS glide bomb | G4.5 | v0.4b | Stand-off precision |
| `talon3` | Talon-3 | AGM, laser, light | G4 | v0.4b | CAS against armour |
| `rkt7` | RKT-7 | 7-tube rocket pod | G3+ | v0.4b | Light aircraft and CAS loadouts |
| `skua1` | Skua-1 | Light anti-ship missile | G4 | v0.5d | Naval forces |
| `tern2` | Tern-2 | Anti-ship sea-skimmer | G4 | v0.5d | Fleet Battle |
| `tt1` | TT-1 | Air-dropped torpedo | G4 | v0.5d | Fleet Battle (surface targets; ASW is M7) |
| `lance6` | Lance-6 | MRM, active radar + datalink | G4.5 | v0.5d | Datalinked shots in networked co-op (§16.5) |
| `pike9` | Pike-9 | LRM, lofted | G5 | v0.5e ✂ | The Wraith; Azure League's G5 kit |
| `dirkc` | Dirk-C | Cruise missile, terrain-following | G4.5 | v0.5e ✂ | Stand-off strike for the bomber role |

| Store | Version | Needed by |
|---|---|---|
| Chaff and flare dispenser loads (internal) | v0.3a | Countermeasures (§17.5) |
| Drop tank `ft1400` | v0.3d | Long-range operations (§49.6) |
| Targeting pod `tp2` | v0.3d | Laser-guided weapons; targeting-pod page |
| Reconnaissance pod | v0.4b | The Reconnaissance mission type |
| Self-protection ECM pod `ecm1` | v0.4b | The `ew_storm` and `decoys` twists; SEAD |
| Stand-off jammer pod `ecm3` | v0.5c | The EW role |
| Towed decoy `td1` | v0.5d | G4.5 defensive kit |

Missile ranges are uncalibrated design targets (Appendix I.6, risk R-24). **Every missile is calibrated in the version that introduces it**, against the hit-rate targets of §15 (`AC-15.5`), by the DLZ baker. **Cut line:** at least 12 weapons at the beta (§66.6.2).

## 3.4 Ground units (Appendix D.3)

| Version | Units | Needed by |
|---|---|---|
| v0.3c | `sam_man` "Sling", `sam_pt` "Gnat", `sam_med` "Halberd", `sam_long` "Bastion"; `aaa_spg`, `aaa_57`; `ewr_1`, `gci_1`, `fcr_1`; `mbt_a`, `mbt_b`, `ifv_1`, `apc_1`, `scout_1`; `cmd_sector`, `cmd_battery`; airbase installations (hardened shelters, hangars, barracks, fuel farm, ammunition depot) | Ground targets of the combat phase (outline §128); §67.2 step 6 (SAM site); the RWR threat library |
| v0.4b | `spg_1`, `mrl_1`; `log_fuel`, `log_ammo`, `log_col`, `log_rail`; `ind_factory`, `ind_refinery`, `ind_depot`; infrastructure targets (bridge, dam, rail yard, power plant, substation, pylon line, communications tower, §47.6) | Convoy Attack, Bombing, Suppression, Dynamic Frontline; IADS command networks (§21.5) |
| v0.5d | No new types; ground forces at scale: ground combat among AI (§21.6), movement and pathfinding on the road graph at battle scale, simulation tiers (§21.3) | The Third Playable's "ground forces" (outline §134) |

## 3.5 Ships (Appendix D.4)

| Version | Ships | Behaviour |
|---|---|---|
| v0.3c | `patrol`; the merchant class (`transport`, `tanker_ship`) | **Static** targets in harbours and on the coast (`SurfaceSim` stub, §61.5) |
| v0.5d | `carrier_large` (CATOBAR), `cruiser`, `destroyer`, `frigate`, and the classes above | Moving ships, ship sensors and point defence, damage and sinking (§22.3–§22.5), task groups (§22.8), the carrier as a mobile airport with catapult and arrester wires (§22.6) |
| v0.7a | `carrier_light` (STOBAR, ski-jump) | The smaller air wing |
| M7 | `sub_1` | Submarines, anti-submarine warfare and port operations (§22.7, §66.7) |

## 3.6 Factions (§37.3)

| Version | Factions | Scope |
|---|---|---|
| v0.3c | **Azure League**, **Meridian Pact** | The `faction/1` data model (§37.2); the two sides of every generated battle; colours and simple insignia |
| v0.4b | + **Concord of Free Ports**, **Iron Marches** | Doctrines feed the generator's force budgeting (§26.8); procedural emblems and liveries with the similarity lint (§37.4); era filter |
| v0.6a | All four | Strategic personalities, relations and the living enemy roster in the campaign (§37.5–§37.6) |

**Cut line:** the campaign may ship with two factions (§66.6.4).

## 3.7 Mission types (§27)

| # | Type | First playable | Generated from | Needs |
|---|---|---|---|---|
| 1 | Quick Dogfight | v0.3b (FP1's single dogfight template) | v0.4a | — |
| 2 | Interception | v0.3d (hand-authored, §67.2) | v0.4a | AWACS AI, GCI |
| 3 | Escort | — | v0.4a | Friendly AI, packages (§20.8) |
| 4 | Strike | v0.3d (hand-authored, §67.2) | v0.4a | Air-to-ground weapons, SAM sites |
| 5 | Air Superiority | v0.3d (hand-authored, §67.2) | v0.4a | — |
| 6 | SEAD | — | v0.4a | Hunter-2 (v0.3d), SAM sites |
| 7 | Survival | — | v0.4a | Reinforcement director |
| 8 | Suppression | — | v0.4b | IADS networks (§21.5) |
| 9 | Convoy Attack | — | v0.4b | Logistics units, road graph |
| 10 | Reconnaissance | — | v0.4b | Scout, reconnaissance pod |
| 11 | Bombing | — | v0.4b | Albatross, heavy bombs and penetrators |
| 12 | Mixed Strike | — | v0.4b | Several objective kinds at once |
| 13 | Boss Hunt | — | v0.4b | Boss and ace system (§29) |
| 14 | Dynamic Frontline | — | v0.4b | Ground forces and frontline rules |
| 15 | Carrier Defense | — | v0.5d | Carriers and naval forces |
| 16 | Fleet Battle | — | v0.5d | Naval forces, anti-ship weapons |

## 3.8 Twists, events, training and arenas

- **Twists (§69.4), all 17 at v0.4b:** `weather_turn`, `defector`, `decoys`, `low_fuel_start`, `awacs_lost`, `ew_storm`, `fog_bank`, `civilian_airliner`, `friendly_in_trouble`, `second_front`, `ambush_valley`, `sam_relocate`, `tanker_damaged`, `runway_cratered`, `late_reinforce`, `comms_degraded`, `boss_cameo`. Each needs a system that exists by then (dynamic weather, civilian air corridors, jamming, AAR and boss cameos are all v0.4b or earlier).
- **Campaign events (§40.5), about 24 at v0.6b:** counterattacks on a forward base or the HQ, raids on fuel and radar, ambushed supply convoys, a weather front or fog closing a base, a named ace sighted, a defector, an allied squadron offering support, equipment shortage, pilot fatigue, militia reports, rival-faction moves and the rest of the initial deck.
- **Training missions (§53.11), at v0.4c:** First Flight; 1 Take-off and landing, 2 Basic manoeuvres and energy, 3 Guns, 4 IR missiles, 5 Radar and BVR, 6 Defensive (RWR, flares, notch), 7 Navigation and air-to-air refuelling, 8 Ground attack. Mission 9, Carrier ops, is post-launch (M7). They are ordinary `MissionDef`s, so they need the mission system (v0.4a), not special code.
- **Curated 1v1 arenas (§31.8), at v0.4c:** Aosta Valley, Khumbu, Yosemite, Nevada basin, Sahara erg, Mid-Pacific, Greenland ice sheet, Norwegian fjords, Tokyo Bay, Atacama. Any location still works; these are bookmarks.
- **Callsigns (Appendix D.6), at v0.3c:** the 120-word sets `raptor_birds`, `weather`, `tools`, `mountains`, and the support callsigns "Watchtower" (AWACS), "Reservoir" (tanker) and "Lighthouse" (ground control).

## 3.9 World content

| Item | Version | Detail |
|---|---|---|
| Test arenas | ✅ v0.1c | `Rugged` and `Plain` on a sphere of R = 6 371 km, with one test airfield (ADR-35, ADR-43) |
| Worldwide coverage | v0.2a | The base globe pack plus streamed tiles for the whole Earth; `AC-3.1` ≥ 95 % at v0.2, ≥ 99 % at v0.4 (§66.3) |
| Pre-packaged regions | v0.2a: 6 | **Pilot set** (decided 2026-11-12, ADR-50): the four terrain-character reference regions of `AC-46.7` (Swiss Alps = Alpine, Nevada Basin-and-Range = Rugged, US Great Plains = Flat, Normandy = Rolling), plus Norwegian fjords (coast and sea) and Tokyo Bay (the dense city for PS-4 and `AC-47.1`) |
| | v0.4: 10 | Adds the remaining 1v1 arena regions that need offline packs |
| | v0.5e: ≥ 20 | The Third Playable's "multiple geographic regions" (outline §134) |
| Airports | v0.2b | Every airport in the merged OurAirports/OSM data, classified into the six types of §10.2, flattened, completed procedurally; improvised strips anywhere (§10.8) |
| Hero-polished airbases | v0.3d: 2, v0.4: 5, v0.5: 10, v0.6: 15, v0.7a: ≥ 30 | Hand-checked layouts, shelters and dressing at real airbases; the rest stay procedural |
| Battlefield sizes (§0.6.4) | v0.2a | All five presets (Small to Extreme) for flight; battle scales follow §0.6.2: Small–Large at v0.4, Huge and Chaos at v0.5d (they need the governor) |
| Weather presets (§24.3) | v0.2c: Clear, Cloudy, Overcast | |
| | v0.4b: + Rain, Storms, Fog, Snow, Heavy winds | Weather fields, fronts and dynamic changes |
| | v0.7a: + Dust | `P10` (sand abrasion) |
| Time of day and astronomy (§24.5–§24.6) | v0.2c | Sun, moon and stars for any date and place |
| Seasons (§46.4) | v0.4b | With the date setting and the full weather |
| Cities and infrastructure (§47) | v0.2c (basic tiers), v0.4b (targets from infrastructure), v0.6b (wartime state) | |

## 3.10 Art and presentation track

The current aircraft and cockpit are generated in code at low polygon counts (owner decision 2026-11-02, ADR-41: 812 triangles against a budget of 8 000). That decision fits a one-aircraft prototype. The GDD's target is far richer: glTF 2.0 models with KTX2 textures, LOD chains and impostors (§12.6), within the budgets of §60.6 (a player-grade aircraft ≤ 90 k triangles plus ≤ 120 k for its cockpit, two 2 K texture sets, ≤ 12 MB per aircraft). OQ-05 (kit-bashed or bespoke) is due before M3. The track below gets from one to the other without blocking the critical path (§66.5: art pipelines start by M1 and are budgeted not to block).

| Version | Art and presentation work |
|---|---|
| ✅ v0.1c | Procedural low-poly Kestrel exterior and cockpit (ADR-41); stroke-font HUD; sky and terrain shading for the arenas |
| v0.2 | **Decide OQ-05.** **Asset-format ADR**: glTF 2.0 + KTX2 loaders of our own or a reopened library choice (ADR-40 names this as its consequence). Budget checker in CI (§60.6). **First hero model: the Kestrel's exterior and cockpit** to player-grade budgets, with the LOD chain and impostor sheet (§12.6) and the canopy, gear and control-surface nodes driven by the sim. Terrain look development and reference boards (G4, `AC-46.5`) |
| v0.3 | Peregrine, Swift, Bittern and Badger as player-grade aircraft with cockpits (kit-bashed if OQ-05 says so); Argus and Stratus at AI grade; weapon and store models (≤ 4 k triangles); explosions, smoke, tracers, wreckage and damage states; ground vehicles, SAM sites and radars (≤ 12 k); the first two hero-polished airbases |
| v0.4 | Condor, Albatross, Scout and Hauler; infrastructure and industrial targets; mask-based liveries and procedural emblems (§14.7, §37.4); photo mode v1; weather visuals (rain, storms, fog, snow) |
| v0.5 | Magpie, Gannet, Wraith and Chimera; the Argus, Condor and Magpie crew stations (multi-crew); ships (≤ 60 k) and the carrier (≤ 250 k) with deck crew and equipment; the full MFD page set (§16.10 tier C) |
| v0.6 | Strategic map and campaign UI art; persistent damage visuals on airbases; squadron emblems |
| v0.7 | Polish pass over every aircraft and cockpit against G4 before the content drop; at least 30 hero-polished airbases; `carrier_light`; the Dust preset |
| M7 | Optional clickable cockpit (§16.10.3, `P10`); new airframes up to the target of 30 (§66.7) |

## 3.11 Audio track

| Version | Audio work |
|---|---|
| ✅ v0.1c | Engine (blade-pass), wind and warning tones, the four buses (ADR-42) |
| v0.1d | Gun, missile launch and seeker tones (the v0.1d audio `weapons` block) |
| v0.3 | Weapons, impacts and explosions; RWR and missile-warning tones; radio v1 with one English voice persona and the D.6 callsigns (§56.6) |
| v0.4 | The full mix: propagation, spatial audio, the worklet synthesis backend (§56.2–§56.5, §61.5); radio with personas and variants; the music director with placeholder stems (§56.8); **G4 audio gate** (aircraft distinctiveness, `AC-56.7`) |
| v0.5 | In-game voice chat (§56.12, OQ-06); localised subtitles (§56.9) |
| v0.7 | Full voice banks per faction; final music; licences audited (G8) |

## 3.12 Localisation

The string architecture and the +40 % length tolerance exist from M1 (§53.10). **The eight target languages (FR, DE, ES, PT-BR, JA, KO, ZH-Hans, RU) are due by M5** (§53.10), so they land at v0.5e with the public beta; voice stays English (brevity codes are English by convention, §56.9). The final language list is an open item (Appendix I.7); v0.7b adds the strings of v0.6–v0.7.

---

# 4. SYSTEM STAGING MATRIX

## 4.1 Every GDD chapter by version

Each cell says what the version adds to the chapter: **stub** (interface and a deliberately simple implementation), **basic** (enough for that version's playable build), **full** (the chapter's specification), or a short description. A dash means nothing changes. Section 19 explains every cell that differs from the chapter's own `P · M` tag.

| § | System | P | v0.1 | v0.2 | v0.3 | v0.4 | v0.5 | v0.6 | v0.7–1.0 | M7 |
|---|---|---|---|---|---|---|---|---|---|---|
| 7 | World and geodesy | P2 | Spherical test arenas | **Full**: geodesy, frames, detail levels, geo service | — | — | — | — | — | Polar support (§45.8) |
| 8 | Battlefield | P2 | Arena bounds | **Full**: shapes, five size presets, boundaries, validator | — | Generator-driven sizing | — | Campaign battlefields | — | — |
| 9 | World map | P2 | — | **Full**: globe, search, coordinates, selection, bookmarks | — | Briefing map | — | Strategic layers (§35.6) | — | — |
| 10 | Airports | P2 | One test airfield | **Full**: data merge, six types, flattening, completion, improvised strips, landing aids | Rearm and refuel capabilities | — | Carriers as airports | Ownership and capture | Hero polish | — |
| 11 | Flight model | P1 | **Full** for the Kestrel (T0) | — | Store and damage effects; new airframes | Reduced-order tiers T1–T3 (§11.17); wind, turbulence, orographic effects | Catapult and arrester forces | — | — | — |
| 12 | Aircraft system | P1 | Data format (cpack) | glTF/KTX2 asset pipeline | Multi-aircraft registry, hardpoints, linter | — | — | Airframe hours and damage | — | Signed custom aircraft |
| 13 | Categories and roster | P1–P3 | Kestrel | — | 5 flyable, balance lint | 7 flyable | 12 flyable | — | Balance from telemetry | Roster to 30 |
| 14 | Customisation and loadout | P3 | — | — | Loadout editor, computed readouts, store limits | Presets and sharing, liveries, insignia, callsign | — | Inventory limits | — | — |
| 15 | Weapons | P3 | Gun, IR missile with simple PN (v0.1d) | — | Seekers, miss model, DLZ baker, air-to-ground, calibration; 13 weapons | 20 weapons | Naval and G5 weapons; 26 | — | Calibration from telemetry | New classes (§66.7) |
| 16 | Radar and avionics | P3 | — | — | Radar, RCS, notch, tracks, RWR, missile warning, terrain LOS (max-mip), datalink, tiers B | LOS through clouds and buildings; tier A | Tier C page set | — | — | — |
| 17 | Electronic warfare | P10 | — | — | Chaff, flares, simple noise jamming | Decoys, self-protection ECM, basic ESM for SEAD | EW console and role, stand-off jamming, towed decoy | — | — | Advanced EW (§17.2 ES/EA/EP trees, deception) |
| 18 | Damage | P3 | Aggregate (v0.1d) | — | Aggregate (FP1), then **component** model; mid-air collisions | — | Ship damage (§22.5) | Persistent airframe damage | — | — |
| 19 | AI | P4 | State-machine bandit, 3 skills (v0.1d) | — | **Utility AI + predictive BFM**, 5 skills, perception, fuel, morale, retreat | Mission-level behaviours (§19.11), radio | Replicated AI at scale | Persistent aces | — | — |
| 20 | Squadrons | P4 | — | — | Formations, wingman orders, GCI/AWACS controller, packages (basic) | Squadron identity, doctrine, coordination | — | Persistent squadrons | — | — |
| 21 | Ground warfare | P3/P5 | — | — | Unit classes, SAM sites, basic networks | IADS with command, convoys, infrastructure, placement hooks | Ground forces at scale, ground combat among AI | — | — | IADS C2 depth (§66.7) |
| 22 | Naval warfare | P10 | — | — | Static ships (stub) | — | **Moving ships, task groups, carriers** | — | Light carrier | Submarines, ASW |
| 23 | Civilian world | P10 | — | — | — | Ambient traffic, civilian air corridors | Road, rail and ship traffic | Campaign effects | — | — |
| 24 | Weather and environment | P2/P3 | Static clear sky | Sky, time of day, astronomy, wind; 3 presets | — | **Full**: 8 presets, weather fields, fronts, dynamic changes, seasons | Wind-field replication | — | Dust preset | — |
| 25 | Mission system | P5 | — | — | Hand-written missions (stub) | **Full**: model, DSL, VM, triggers, rule sets | Co-op rule sets | Operations | — | DSL scripting expansion |
| 26 | Mission generator | P5 | — | — | Early templates | **Full**: pipeline, seeds, placement, budgeting, solvability | `roles_present` | Strategic inputs | — | — |
| 27 | Mission types | P5 | — | — | 4 | 14 | 16 | — | — | — |
| 28 | Escalation | P5 | — | — | Reinforcement waves (basic) | **Full** director | — | — | — | — |
| 29 | Bosses and aces | P5 | — | — | — | Single-session bosses | Multiplayer scaling (§29.7) | Persistent identities | — | — |
| 30 | Sandbox configuration | P2/P5 | — | v1: place, size, time, weather | Forces and AI skill | **Full** | Chaos mode (needs the governor) | — | — | — |
| 31 | 1v1 | P8 | — | — | — | Against the AI on one machine, curated arenas | **Online**, rule sets, spectating | — | — | Tournament tooling |
| 32 | LAN/VLAN co-op | P7 | — | — | — | — | **Full**: lobby, roles, multi-crew, voice | — | — | Co-op campaign (§32.11) |
| 33 | Multiplayer architecture | P7 | Loopback ✅ | — | — | — | **Full** | — | — | 32 humans, spectator mode |
| 34–43 | Campaign | P9 | Interfaces reserved | — | — | — | — | **Full** | Content and tuning | New operations, daily seeds |
| 44 | World streaming | P2 | Local patch | **Full** | — | — | Tile relay by the host | — | — | Higher caps |
| 45 | Geographic data | P2 | — | **Full** pipeline, GeoPack, offline cache, licensing; 6 regions | — | 10 regions | ≥ 20 regions | — | Release set | More regions |
| 46 | Terrain and biomes | P2 | Arena shading | **Full**: shading, biomes, water, vegetation, geography as gameplay | — | Seasons | — | — | — | Seasonal styles |
| 47 | Cities and infrastructure | P2–P3 | — | Basic tiers | — | Targets from infrastructure | — | Wartime state | — | Better coverage |
| 48 | Airbase operations | P3 | Runway start | — | **Core**: start types, scramble, ground operations, rearm, refuel | Alert levels and base C2 for the generator | Carrier deck operations | Persistent damage and repair | — | — |
| 49 | Long-range operations | P2–P3 | — | — | Time acceleration, cruise, autopilot holds, AAR, fuel planning | Intercept in transit | — | — | — | — |
| 50 | Navigation | P2 | — | **Basic**: waypoints, bearing and distance, position, holds | **Full**: autopilot modes, TACAN/ILS-like aids | Realism gating | — | — | — | — |
| 51 | User-generated content | P5 | — | — | — | Seed links, configuration sharing, file import/export | Editor v1, user battlefields | — | — | Community hub (OQ-10) |
| 52 | Replay and photo mode | P5 | — | Ring buffer (stub) | — | SSR files, kill cam, photo mode v1 | OSR, replay viewer, multiplayer rules | — | Video capture, cinematic director | Editing timeline |
| 53 | UI/UX | P1–P10 | Shell, settings, first run ✅ | World map UI | Minimal mission flow, debrief | **Full flow**: briefing, loadout, debrief, training, hints, encyclopaedia | Lobby; 8 languages | Campaign UI | Polish | Mobile research |
| 54 | HUD and cockpit | P3 | Core HUD, cameras ✅; gun sight (v0.1d) | — | Full symbology (DLZ, RWR, radar); tier B MFD pages | Tier A | **Tier C page set**, crew stations | — | — | Clickable cockpit |
| 55 | Tactical map | P3 | — | **Basic** | Tracks and threats | **Full** information model | Shared picture | Campaign fog | — | — |
| 56 | Audio | P3 | Engines, wind, tones ✅; weapons (v0.1d) | — | Weapons, RWR, radio v1 | **Full mix**, spatial, radio personas, music director | Voice chat, subtitles | — | Voice banks, music | — |
| 57 | Accessibility | P1–P10 | Baseline settings ✅ | — | — | **G7 baseline** | **Complete**, external review 1 | — | External review 2 | — |
| 58 | Controls | P1 | **Full** ✅ | — | Weapon and sensor actions | Head tracking (§58.7) | — | — | — | Force feedback (§58.9), WebXR (OQ-07) |
| 59 | Difficulty and realism | P3–P10 | Assist levels ✅ | Five presets | Avionics tiers in the presets | **All dials**, Realism Index | Multiplayer locks | Campaign difficulty | — | — |
| 60 | Performance | P1–P10 | Budgets and arena gates ✅ | PS-3, PS-4, PS-6, PS-8 | — | PS-1, PS-7 | PS-2, PS-5, governor | — | Full regression | Higher caps |
| 61 | Technical architecture | P1–P10 | ✅ (v0.0) | — | — | — | Dedicated server | — | Security audit | Desktop wrapper (OQ-09) |
| 62 | Data-driven content | P1–P10 | Registry, aircraft, weapons ✅ | Geo content | — | All content types; `NoHardcodedContentLint` | — | Campaign content | — | Signed packs |
| 63 | Debugging tools | P1–P10 | Core tools ✅ | World panels | AI debugging | Mission and generator panels | Network panel, impairment | Campaign inspector | — | — |
| 64 | Save system | P2–P9 | Settings and profile ✅ | Checkpoints | — | — | — | **Full** | — | — |
| 65 | Statistics | P5–P9 | Debrief stub (v0.1d) | — | — | Mission statistics, logbook | — | Career, campaign | — | — |
| 69 | Anti-repetition | P5 | — | — | — | **Core** | — | Campaign integration | — | — |
| 70 | Experience tests | — | — | — | — | ET-1 | ET-3 | ET-2 | All re-run | — |

## 4.2 Extension seams (§61.5)

| Capability | Interface | Stub | Real implementation | §61.5's milestone | This roadmap |
|---|---|---|---|---|---|
| Weather | `WeatherProvider` | Static layers + uniform wind ✅ (v0.1c) | Dynamic fronts, turbulence, orographic effects | M4 | v0.4b |
| Terrain heights | `GeoService` | Procedural + base heights ✅ (v0.0b) | Streamed GeoPack | M2 | v0.2a |
| Line of sight | `LosService` | Terrain-only ray-march | Max-mip + horizon profiles; then clouds and buildings | M3 / M4 | Max-mip terrain LOS **v0.3a** (§66.3 M3); clouds and buildings v0.4b (R3) |
| Aerodynamics | `AeroModel` | Table lookup ✅ (v0.1a, ADR-29) | Store and damage effects | M3 | v0.3c–d |
| Damage | `DamageModel` | Aggregate ✅ (v0.1d) | Component and subsystem model | M3 (was M4; GDD 0.2.5) | **v0.3c** (§66.3 M3, R3) |
| Sensors | `RadarModel` | Range/RCS threshold | Radar equation, Doppler, notch, ECM | M3 (was M4; GDD 0.2.5) | **v0.3a** (§66.3 M3); FP1 uses its simplified modes (§67.1, R3) |
| Missiles | `MissileModel` | Simple PN ✅ (v0.1d) | Seekers, guidance, DLZ | M3 (was M4; GDD 0.2.5) | **v0.3a** (§66.3 M3, R3) |
| AI pilot | `PilotBrain` | State machine ✅ (v0.1d) | Utility AI + predictive BFM | M3 (was M4; GDD 0.2.5) | **v0.3c** (§67.2 "better AI", R3) |
| Mission source | `MissionProvider` | Hand-written missions | Generator, escalation, bosses | M4 | Stub v0.3b; real v0.4a–b |
| Persistence | `PersistenceSink` | In-memory ✅ | Campaign save and ledger | M6 | v0.6a |
| Strategic layer | `StrategicContext` | Stub ✅ | Campaign | M6 | v0.6a |
| Naval | `SurfaceSim` | Static ships | Moving ships, carriers | M3 / M5 | v0.3c / v0.5d |
| Network | `Transport` | Loopback ✅ | WebSocket LAN, WebRTC, WebTransport | M5 | v0.5a |
| Rendering | `Renderer` | WebGPU and WebGL2 basic ✅ (custom, ADR-40) | Full WebGPU stack | M2+ | v0.2 onward |
| Audio | `AudioBackend` | Beeps and samples ✅ (v0.1c) | Worklet synthesis + spatial | M4 | v0.4c |
| Replay | `ReplaySink` | Ring buffer in memory | SSR and OSR files | M2 / M5 | Ring buffer v0.2d; SSR v0.4c; OSR v0.5e |

---

# 5. v0.0: Foundations (M0, about 2 months)

**Goal:** prove the architecture end to end with the least possible content, so that every later version is "replace a stub", not "rewire".

## 5.1 v0.0a: Repository, build, CI (about 4 weeks)

- Monorepo layout (§61.3): Rust workspace, TypeScript workspace, `/content`, `/tools`, `/server`, `/docs`.
- Build pipelines: Rust → WASM (threads and single-thread builds), TS bundling, WGSL validation, content-addressed outputs.
- CI: lint, unit tests, native and WASM builds, **cross-build state-hash test** (ADR-10), reproducible-build check.
- ADR repository seeded with ADR-01…20 (§0.4); open questions OQ-01…10 assigned owners.
- Staging deploy with COOP/COEP headers and Service Worker shell (§61.12).

**Exit:** green pipeline; staging URL serves a cross-origin-isolated page; WASM and native hashes equal after 10⁶ ticks.

## 5.2 v0.0b: The empty loop (about 4 weeks)

- Sim Worker: fixed 60 Hz tick, drift-correcting accumulator, snapshot buffer (seqlock) (§0.6.1, §61.2).
- Render Worker: WebGPU on `OffscreenCanvas` with WebGL2 fallback; lit terrain patch; camera-relative f32 rendering (§44.8).
- Input ring and shaping pipeline (§58.2); compat-check screen (§53.3); debug overlay (FPS, stage timings) (§63).
- Content baker stub with one aircraft file; extension seams with stub implementations (§61.5).
- Performance harness skeleton (§60.9); first-run benchmark prototype (§60.8).

**Exit:** sim runs stably at 60 Hz in a worker while the render runs independently; runs on the three reference machines (Minimum / Recommended / High).

**Status (2026-10-14):** all work packages are merged to `main` and green in CI, and the scripted reference runs on the Mac (A: WebGPU, B: forced WebGL2) pass. The exit is **open on two reference runs on the project owner's laptop**: C (RTX 5060, WebGPU, High) and D (Chrome forced onto the integrated GPU, Minimum-class GPU). The owner will send those reports later; the `v0.0b` tag waits for them. Steps and pass criteria: [docs/runbooks/reference-machines.md](runbooks/reference-machines.md). Evidence so far: [docs/exit/v0.0b.md](exit/v0.0b.md).

**Status (2026-11-12): closed by the owner's waiver.** On 2026-10-16 the owner waived the laptop runs ("just override v0.0b, I don't think we need to know that"), and v0.1a went on without them. Runs C and D were never made, and no `v0.0b` tag exists. The Minimum-class hardware gap stays as ADR-28 records it.

## 5.3 Delivered by v0.0

| Area | What exists |
|---|---|
| Runtime | Four-role threading model (ADR-17), sim loop pacing (ADR-26), runtime topology and ABI (ADR-25) |
| Platform | GitHub Pages staging with Service Worker isolation (ADR-22); PWA shell (ADR-01) |
| Quality | State-hash and golden policy (ADR-23); seams policy and own math types (ADR-27); performance gating and reference machines (ADR-28) |
| Rendering | Thin custom renderer over WebGPU and WebGL2 (ADR-24) |

---

# 6. v0.1: Flight Prototype (M1, about 4 months)

**Goal:** "The aircraft must feel good to fly before adding extensive content" (outline §126). Priority P1.

## 6.1 v0.1a: Flight core (about 5 weeks)

- 6-DOF rigid body with quaternion attitude, RK4 at 120 Hz for T0 (§11.2, §11.10).
- ISA atmosphere and air data (§11.3, §11.5); table-driven aerodynamics (§11.6) and propulsion (§11.8); mass properties (§11.9).
- Kestrel data (Appendix C) and the **Performance Conformance Test** harness (§11.19), running headless in CI.

**Exit:** the point-performance targets pass within ±3 % / ±5 % (`AC-3.2`); WASM vs native hashes equal for a 60 s scripted flight (`AC-11.7`).

**Note (2026-10-14):** the conformance method is fixed in [ADR-31](adr/0031-performance-conformance-test-method.md). The two ground-roll points of the envelope need the gear model, so at v0.1a they are reported as deferred and `AC-3.2` passes on the ten airborne points; it closes fully at v0.1b. Work is on branch `v0.1a` and merges to `main` only after `v0.0b` is tagged.

**Status (2026-10-15):** both exit criteria pass on branch `v0.1a` (draft PR #5). In the conformance run, the ten airborne points use at most 35 % of their tolerance, and flight60 hashes equal across native, debug, WASM single and WASM threads at all 61 checkpoints. The real Kestrel replaces the observer as the player's aircraft, flown with direct surface control until v0.1b. Still open: the merge (waiting for the `v0.0b` tag), the performance re-baseline with runs A and B against the new staging, and the exit checklist ([docs/exit/v0.1a.md](exit/v0.1a.md)).

## 6.2 v0.1b: Control and ground (about 4 weeks)

- Flight control system: Full / Standard / Reduced / Manual assist (§11.11); limiters (α, g, overspeed).
- Ground handling: gear model, friction, take-off and landing, crash criteria (§11.13).
- G-physiology and stall/departure behaviour (§11.14, §11.15).
- Keyboard, mouse (mouse-aim), gamepad and HOTAS input with binding editor v1 (§58).

**Exit:** a tester can take off, fly a pattern and land under every assist level; input-to-control latency ≤ 25 ms p95; the take-off and landing ground-roll conformance points deferred from v0.1a pass (`AC-3.2` complete).

**Note (2026-10-22):** the gates are fixed before the work in [ADR-32](adr/0032-ground-roll-conformance-method.md) (ground-roll method; the landing-roll target is kept and the data retuned) and [ADR-33](adr/0033-v0-1b-gates.md) (virtual-tester circuit under every assist, FCS handling and departure suites, latency definition, mouse-aim criterion scaled to the aircraft's turn rate). "A tester" is a scripted virtual tester run headless in CI; the owner's manual circuits on staging are recorded and do not block the tag. The scope is larger than the 4-week slot (about 13 engineer-weeks): expect the slot to slip. Work is on branch `v0.1b`.

**Status (2026-10-27):** the code is merged to `main` (PR #7, with the required checks green and the staging smoke test passing); the exit checklist is [docs/exit/v0.1b.md](exit/v0.1b.md). In the headless gates, `AC-3.2` is complete (12 of 12 points, the deferred set empty), the virtual tester flies take-off, circuit and landing under all four assist levels, and the three determinism suites hash equal across native, debug and WASM. The reference-machine runs (2026-10-28, Mac M4 on mains power) pass: control latency p95 17.25 ms over 567 changes against the 25 ms gate, and the PS-0 strict gates on WebGPU and on WebGL2 at 4x throttle. Open: merge 2 and the tag, and the owner's manual circuits (they do not block). The slot slipped as expected.

## 6.3 v0.1c: Presentation (about 4 weeks)

- Cockpit and chase cameras (§54.11); core HUD symbology and collimation (§54.2, §54.3).
- Procedural test arenas plus basic terrain (low-resolution global + local patch) (§44, basic).
- Engine, wind and warning-tone audio (§56 basic).
- First-run flow, settings, PWA install and offline shell (§53 minimal).

**Exit:** 60 FPS on the Recommended profile in the arena; horizon dip and curvature correct in screenshot tests.

**Note (2026-11-03):** the gates are fixed before the work in [ADR-39](adr/0039-v0-1c-gates.md): measured horizon dip and curvature in screenshot tests (with a seeded flat-Earth fault that must fail), HUD conformity and collimation, camera terrain clearance, objective audio checks (blade-pass frequency, loudness, warning onset), and the arena frame-rate gates. Owner decisions (2026-11-02): the renderer stays custom ([ADR-40](adr/0040-custom-renderer-stays.md), superseding ADR-18); "the Recommended profile" is the Mac M4 on WebGPU, with the benchmark's CPU thresholds re-based by v0.1a's method so it reads High again ([ADR-28](adr/0028-perf-gating-and-reference-machines.md) amendment 1); the aircraft and cockpit are procedural low-poly; the automated gates decide the tag and the owner's look-and-listen checklist does not block. The scope is larger than the 4-week slot (about 14.5 engineer-weeks): expect the slot to slip. Work is on branch `v0.1c`.

**Status (2026-11-08): done and tagged** (`v0.1c` on `1124d43`: the code is PR #11, the evidence and documents PR #12, a fix to the staging smoke test PR #13; the pipeline and the staging deploy and smoke test are green on that commit); the exit checklist is [docs/exit/v0.1c.md](exit/v0.1c.md). The arena holds the display's 120 FPS at 1920 x 1080, High, on the M4 (WebGPU) in the cockpit and the chase view with the HUD and sound on, frame interval p99 9.5 ms against a gate of 22.2 ms; the horizon's dip is within 0.05 degrees at five heights in CI and on the M4's GPU, with a seeded flat-Earth fault that fails as it must; the three older determinism goldens are unchanged. The reference runs were taken on battery power first (the gate says mains) and repeated on mains after the tag against the deployed build, with the same verdict. S6 (SSIM image goldens) was cut. The fault runs found the audio kernel allocating 23 KB a block (rewritten in passes) and three gates that could not fail (A1, A7 and the collimation twin: strengthened); the owner's look-and-listen checklist is open and did not block the tag.

## 6.4 v0.1d: Basic combat stub and the feel gate (about 3 weeks)

- One gun and one IR missile (simple proportional navigation); one AI bandit (state machine) that pursues and defends.
- Debrief and statistics stub (§65 basic).
- **Gate G1 (flight feel)** with ≥ 12 blind testers (§66.4); 30-minute soak with no leaks.

**Exit:** G1 passes; ≥ 80 % of external playtesters take off, fly, shoot a bandit and land within 10 minutes (`AC-4.5` precursor).

**Note (2026-11-09):** the gates are fixed first in [ADR-45](adr/0045-v0-1d-gates.md), before any weapon, AI or combat code. Owner decisions (2026-11-08): **D1** the automated gates decide the tag (the conformance suite, an audit of the flight-control system, and a bot playtester that takes off, kills a bandit and lands under every assist level), the owner's own ratings are recorded and do not block, and the 12-tester blind study is dropped from this milestone and kept open as OQ-11; **D2** the weapon and AI data go into the sim pack and the four existing goldens are re-blessed once, proven by a checker that only the `content` and `combined` columns changed; **D3** the bandit fires its gun by skill (the Recruit never does) and has no missiles. The scope is larger than the 3-week slot (about 13.6 engineer-weeks): expect the slot to slip. Work is on branch `v0.1d`. The exit checklist is [docs/exit/v0.1d.md](exit/v0.1d.md).

**Status (2026-11-11): done on bot evidence**, with G1(a) open as OQ-11; the tag `v0.1d` goes on merge 2's commit once its pipeline and staging are green. The code is PR #15 (merge 1, `261d08b`), the evidence and documents merge 2; the exit checklist is [docs/exit/v0.1d.md](exit/v0.1d.md). One gun and one rear-aspect IR missile ([ADR-46](adr/0046-weapons.md)), one bandit at three skills and the bot playtester ([ADR-47](adr/0047-combat-ai-and-the-bot.md)), combat in the core with ABI v5 ([ADR-48](adr/0048-combat-in-the-core.md)), and the Dogfight mode, debrief, statistics and feel questionnaire ([ADR-49](adr/0049-debrief-statistics-and-the-feel-questionnaire.md)). The bot takes off, shoots a Recruit down and lands in 44, 41 and 43 of 50 sorties under Standard, Reduced and Manual (gate 40; Full 37, informational by owner decision); the NN-2 audit shows the control system never moves the aircraft. On the M4 the dogfight holds the display's 120 FPS at 1920 x 1080, High, frame interval p99 9.5 ms, with control latency p95 16.75 ms during a duel; the 30-minute soak passes all of L1 to L7 on the M4 (60.02 Hz, a heap slope of 0.03 MB per 10 minutes, and no growth in DOM, WebAssembly memory, GPU buffers or audio nodes over 7 sortie starts) and in CI's nightly. The four older determinism goldens moved only in their `content` column (one proven re-bless). The pre-merge walkthrough found the bandit's gun unable to hit (it chased its own pipper); it was fixed before merge 1. The owner's checklist is open and does not block.

**Status (2026-11-11): tagged.** The annotated tag `v0.1d` is on `dab452f` (merge 2, PR #16). Its pipeline ([run 43082009046](https://github.com/Bl4202/Flying/actions/runs/43082009046)) is green, including deploy-staging and staging-smoke. The full 30-minute soak on it in CI's nightly ([run 43082063829](https://github.com/Bl4202/Flying/actions/runs/43082063829)) passes L1 to L7 (59.99 Hz, 7 sortie starts).

## 6.5 Delivered by v0.1

| Area | Content |
|---|---|
| Aircraft | Kestrel: `aircraft/1`, `aero/1` tables, the `f129` engine, the FCS gains, the procedural model and cockpit, the HUD layout, the audio profile (`content/core/`) |
| Weapons | GS-27 gun, Dart-1 IR missile (v0.1d) |
| AI | The bandit at three skills: Recruit, Regular, Veteran (`content/core/skills/`) |
| World | Two test arenas (`Rugged`, `Plain`) with one test airfield (ADR-35, ADR-43) |
| Controls | Four assist levels; keyboard, mouse-aim and mouse-stick, gamepad and HOTAS profiles; binding editor (ADR-37) |
| Presentation | Cockpit and chase cameras, the core HUD, engine, wind, warning and weapon sounds |
| App | First-run setup, main menu, settings (`settings/2`), PWA install and offline shell (ADR-44) |

## 6.6 Milestone M1 exit (§66.3) and what carries over

| M1 exit item | Where it stands |
|---|---|
| G1 (a) blind panel | Deferred by D1; open as OQ-11 |
| G1 (b) conformance | ✅ `AC-3.2` 12 of 12 |
| G1 (c) no fixed-rate-turn fallback (NN-2) | ✅ v0.1d (ADR-45 G1-c1 and G1-c2) |
| `AC-3.2` | ✅ |
| 60 FPS on Recommended in the arena | ✅ v0.1c; v0.1d in the dogfight (PF1) |
| Input-to-control ≤ 25 ms p95 | ✅ v0.1b (17.25 ms); v0.1d with a duel running (16.75 ms) |
| 30-minute soak with no leaks | ✅ v0.1d (the M4 and the nightly run) |
| `AC-4.5` precursor | ✅ v0.1d (the bot playtester, BP1, D1) |

**Carried forward from M1:** the hero Kestrel model (v0.2c, section 3.10); head tracking (§58.7, part of M1's chapter but not of M1's scope in §66.3; now v0.4c); the owner's look-and-listen checklist from v0.1c and the owner's feel questionnaire from v0.1d (recorded, not blocking). *2026-11-12:* the owner's four hands-on checklists of v0.1a to v0.1d stay open and non-blocking; they are now one sitting, "The owner's M1 session", in [docs/runbooks/reference-machines.md](runbooks/reference-machines.md).

---

# 7. v0.2: Geographic Sandbox (M2, about 5 months)

`M2 · P2 · outline §127 · about 5 months · planned` — "The player should be able to select two geographic locations and fly between them" (outline §127). The sandbox becomes the whole Earth: globe, coordinates, location selection, terrain streaming, airports, battlefield generation and boundaries.

## 7.0 Entry, decisions and scope

- **Entry:** v0.1d tagged ✅ (2026-11-11, `dab452f`). G1(a) staying open does not block v0.2 (ADR-45, D1).
- **Decisions due before M2:** OQ-03 (production hosting, range-request CDN, who pays for tile bandwidth) and OQ-04 (commercial or free release; it decides which attribution and share-alike duties apply to the datasets, §45.9). Both shape the GeoPack delivery model of v0.2a. **Decided 2026-11-12 ([ADR-50](adr/0050-entering-m2.md)):**
  - **OQ-03:** the app stays on GitHub Pages and the GeoPacks are served from a Cloudflare R2 bucket.
  - **OQ-04:** kept open until the public beta. Until then, packs take only data whose licence allows commercial use.
  - **The six pilot regions:** as section 3.9.
  - **OQ-02:** closed (solo).
- **Decisions due during v0.2 (before M3):** OQ-05 (art budget: kit-bashed or bespoke) and OQ-07 (WebXR stance).
- **Not in v0.2:** combat on real terrain (v0.3b; the v0.1d duel keeps working in the test arenas), the mission generator, dynamic weather (v0.4b), civilian traffic (v0.4b), multiplayer.

## 7.1 v0.2a: Globe, frames and the pipeline (about 6 weeks)

**Goal:** the Earth exists as data, streams without holes and renders from orbit to the runway.

- **Geodesy and frames** (§7.2–§7.7): geodesy primitives, distances and bearings, the curved battlefield frame (ADR-04), altitude definitions, the world clock, the sim-side geo service; levels of geographic detail (§7.1).
- **Battlefield system** (§8.1–§8.8): shapes, the playable area against the visible horizon, vertical limits, boundary enforcement, the five size presets of §0.6.4, airport-corridor sizing, the anchor and local frame, the validator.
- **Geodata pipeline** (§45.5–§45.7, §45.11): the offline `geo-pipeline` (ingest, normalise, water flattening, tile, airport patches, quantise and pack, features, indexes, archive, publish), the `.gpk` GeoPack format, vertical datum normalisation (ADR-05), query services and indexes, versioning and updates.
- **Licensing** (§45.9, Appendix H): source descriptors, attribution in the UI, the licence audit in every pack build (which also refuses a descriptor with `commercial_ok: false` while OQ-04 is open, ADR-50).
- **Hosting** (ADR-50): the GeoPacks in a Cloudflare R2 bucket, with range requests and CORS (the page is isolated with COEP `require-corp`, ADR-22); an upload job in CI with a scoped token.
- **Streaming** (§44.2–§44.10): streaming levels, quadtree and tile identity (ADR-06), level-of-detail selection, the streaming pipeline and its residency invariant, failure handling, precision, concurrency.
- **Rendering:** globe rendering and atmosphere (§9.9, §24 basic); the `GeoService` seam's real implementation (§61.5).

**Content:** the base globe pack (the whole Earth at low detail, streamed tiles beyond it) and the six pilot regions at full detail (section 3.9).
**Tools and tests:** coverage and QA reports per pack build (void percentage, error histograms, missing tiles, licence audit); world debugging views for tiles and residency (§63.7).
**Exit:**
1. Seamless zoom from orbit to the ground; no holes in a scripted descent (`AC-44.1` precursor).
2. Geodesy round-trip tests (`AC-7.*`) and battlefield validator tests (`AC-8.*`) pass on native and WASM.
3. The licence descriptor audit passes for the pilot pack (`AC-45.4`).

**Note (2026-11-12):** the gates are fixed first in [ADR-51](adr/0051-v0-2a-gates.md), before any geodesy, battlefield, pipeline, streaming or world-rendering code. Owner decisions (2026-11-12): **D1** the whole Earth streams to z10 (about 150 m posts) from Copernicus DEM GLO-90, and the six pilot regions to z12 (about 38 m posts) from GLO-30; worldwide z11–z12 is v0.2b's; **D2** the datapack is built and uploaded to R2 by a manual GitHub Actions workflow with a scoped token kept as repository secrets; **D3** the bucket is read through its `r2.dev` address until a custom domain before the public beta (v0.5e); **D4** the six regions are in the main menu's Arena list (air start, a Medium battlefield with its boundary) beside a Globe view, and the test arenas and the dogfight stay as they are. The five existing determinism goldens must not change. The scope is larger than the 6-week slot (about 19.5 engineer-weeks): expect the slot to slip. Work is on branch `v0.2a`. The exit checklist is [docs/exit/v0.2a.md](exit/v0.2a.md).

## 7.2 v0.2b: World map and airports (about 6 weeks)

**Goal:** the player picks any place on Earth, and every real airport is a usable place to start.

- **World map** (§9, §53.5): camera model, picking, layers and overlays, offline place-name search, coordinate entry, selection tools (rectangle, circle, the capsule between two airports, ruler), bookmarks, the political depiction policy (§9.8, OQ-08). The first interactive screen asks "Where do you want to fight?" (`AC-71.2`).
- **Airports** (§10): the data model, the six airport types and their classification from data, runways and surfaces, layouts and ground networks, procedural completion, pad flattening baked into the tiles, improvised strips anywhere (§10.8), navigation-aid and landing-assist data (§10.9), airport state.
- **Determinism with streaming:** the sim terrain layer (§44.6, ADR-15): the simulation reads only the base DEM, so the state hash does not depend on what has streamed.
- **Spawning:** the runway start at any real airport replaces the test airfield; air starts anywhere.

**Exit:**
1. `AC-10.1`: ≥ 98 % of airports usable.
2. `AC-3.1`: ≥ 95 % of 1 000 random locations yield a playable battlefield.
3. The world map meets its budgets (§9.11) and its acceptance criteria (§9.12).
4. A scripted flight over streamed terrain hashes equal on native and WASM.

## 7.3 v0.2c: Terrain, world content and the look (about 5 weeks)

**Goal:** the terrain is recognisable from 10 km up and believable at 30 m (§46 intent), and the first hero aircraft model replaces the procedural one.

- **Terrain and biomes** (§46.1–§46.10): landcover classes, biome rules, shading and materials, procedural detail and upsampling, vegetation and ground cover, water, missing-data fallback, terrain features that matter for gameplay.
- **Cities and infrastructure, basic tiers** (§47.1–§47.5, §47.7): footprints and heights from data with procedural facades, building rendering tiers, roads, rail, bridges, power lines, spatial structures.
- **Sky and environment** (§24.3, §24.5, §24.6, §24.8): time of day and date, sun, moon and stars, the sky and cloud layers, three weather presets (Clear, Cloudy, Overcast), uniform wind.
- **Sandbox configuration v1** (§30): location, battlefield size and shape, date and time, weather preset, start type.
- **Art** (section 3.10): the asset-format ADR (glTF 2.0 + KTX2), the §60.6 budget checker in CI, **the hero Kestrel exterior and cockpit** with its LOD chain and impostor; terrain look development.

**Exit:**
1. Look-development reference boards reviewed (G4 look-dev, `AC-46.5`).
2. The building budget holds in the dense-city scenario (`AC-47.1`); PS-4 (downtown, 40 k buildings, night: ≥ 55 FPS, ≤ 1.2 M building triangles).
3. The hero Kestrel is within the §60.6 budgets and the arena frame gates of v0.1c still pass.

## 7.4 v0.2d: Navigation, load plan, cache and the two-location test (about 4 weeks)

**Goal:** the outline's test: choose two places, fly from one to the other, on a cold cache and offline.

- **Loading** (§44.9, §45.4): the load plan, prefetch and pipeline cache; the OPFS cache, persistence requests (ADR-19) and the Data & Storage screen.
- **Failure matrix and fault injection** (§45.10, §63.8).
- **Geography as gameplay hooks** (§46.11): the terrain-character metrics `M`, `R`, `O` and their classes (`AC-46.7`).
- **Basic navigation** (§50.2, §50.3, §50.6): the position source, heading, bearing and distance, waypoints placed on the map; heading and altitude holds (part of §50.4).
- **Basic tactical map** (§55): own ship, the battlefield boundary, airports and waypoints.
- **Difficulty presets** (§59.3): the five presets, mapping the dials that exist (assist level, g-effects, stall protection, ground handling).
- **Saving** (§64): checkpoints and quick-save in free flight.
- **Replay seam** (§61.5): the in-memory ring buffer.

**Exit:**
1. **The two-location test** passes on a cold cache and again offline after caching.
2. Streaming budgets met (`AC-44.4`, `AC-44.5`); PS-3 (Mach 1.0 at 150 m AGL over mountains, cold cache, 20 minutes: no holes, streaming ≤ 0.7 ms average).
3. PS-8 load times within §0.6.6; PS-6 (Minimum profile, Low preset, 900p) in its flight-only form.
4. `AC-46.7`: the four reference regions classify as Alpine, Rugged, Flat and Rolling.

## 7.5 Milestone M2 exit (§66.3)

The two-location test with continuous streaming and no holes (`AC-44.1`); `AC-3.1` ≥ 95 %; horizon dip and curvature correct on streamed terrain (first proved in the arena at v0.1c); streaming budgets (`AC-44.4`); airport merge (`AC-10.1`).

## 7.6 Risks

Streaming holes and hitches (mitigation: the residency invariant and the sim terrain layer, §44.5, §44.6); data licensing (OQ-04, the audit in every build); tile bandwidth and hosting cost (OQ-03); WebGPU driver variance on the Minimum profile (PS-6).

---

# 8. v0.3: Combat (M3, about 6 months)

`M3 · P3, P4 · outline §128, §132, §133 · about 6 months · planned` — Guns, missiles, radar, RWR, countermeasures, AI, ground targets and damage that work well (outline §128). This version contains the two earliest playable builds.

## 8.0 Entry, decisions and scope

- **Entry:** v0.2 tagged.
- **Decisions due before M3:** OQ-05 (art budget) and OQ-07 (WebXR stance), and the recruiting decision for the G2 panel (section 1.6).
- **Not in v0.3:** the mission generator beyond early templates (v0.4a), moving ships and carriers (v0.5d), dynamic weather (v0.4b), multiplayer, campaign, replay files, photo mode, advanced EW.

## 8.1 v0.3a: Weapons and sensors (about 7 weeks)

**Goal:** weapons and sensors whose outcomes emerge from physics, not from dice (NN-3).

- **Weapons** (§15): the guns and ballistics of v0.1d generalised (§15.4); the missile flight model at game fidelity (§15.5); the miss model and lethality (§15.6); seekers: IR, imaging IR, semi-active and active radar (§15.7); countermeasure interaction (§15.8); release and launch logic (§15.10); the **DLZ baker** and the calibration of every missile against the §15 hit-rate targets (risk R-24).
- **Radar and sensors** (§16.3–§16.9): radar modes, the radar equation, Doppler, notch and look-down, track-while-scan; RCS by aspect; tracks, track quality and identification; passive and visual detection; missile-approach warning; the RWR; the line-of-sight service with max-mip terrain masking. Sensors and AI never read the render world (ADR-16).
- **Countermeasures** (§17.5): chaff, flares and dispenser programmes; simple noise jamming (§17.3 basic).
- **HUD and displays** (§54.4–§54.6, §16.10): weapon display and launch-zone indicators, RWR, radar and notch indicators; the radar and stores pages of avionics tier B.
- **Controls** (§58.5): the weapon and sensor actions.

**Content:** Lance-4, Lance-2, Dart-4; chaff and flare loads; RCS and signature data for the Kestrel.
**Exit:**
1. Hit-rate sweeps within targets, with the monotonic ordering of `AC-15.5`.
2. Line of sight agrees with the reference (`AC-16.*`); sensor budgets met (§16.12).
3. The NN-3 lint finds no auto-hit path.

## 8.2 v0.3b: First Playable Build (about 5 weeks)

**Goal:** prove the central concept: choosing a place on Earth and fighting there is fun (§67.1, outline §132).

**The 11-step script (§67.1):**

| # | Step | Acceptance condition | Time budget |
|---|---|---|---|
| 1 | Open a geographic map | Globe visible, rotate and zoom work, search works | ≤ 3 s cached |
| 2 | Select a location | Coordinates shown, battlefield footprint preview | ≤ 1 s feedback |
| 3 | Generate a battlefield | Medium (120 km), boundary validated, two airports found or improvised | ≤ 6 s cached, ≤ 12 s streamed |
| 4 | Load terrain | Load plan complete, sim-critical tiles resident | ≤ 25 s to launch |
| 5 | Spawn an aircraft | Kestrel on the runway (hot start) or in the air | Immediately after load |
| 6 | Fly | Take off, climb, turn and land; curvature and horizon correct | — |
| 7 | Spawn an enemy | One AI bandit (G4 fighter, Veteran) at the boundary or from an enemy base | 60–120 s after launch |
| 8 | Detect it | Radar (simplified modes) or visual + RWR cues; HUD target box | — |
| 9 | Fire a missile | Launch with DLZ cues: one radar missile (Lance-4) and one IR missile (Dart-1) | — |
| 10 | Destroy it | Hit and kill with a proper explosion, wreckage and kill feedback | — |
| 11 | Return to the map | Land (optional) or end; debrief; the globe re-opens | ≤ 4 s |

- **Included** (§67.1): §7–§10 basic, the Kestrel, two missiles and the gun, simplified radar and the RWR, aggregate damage, the bandit at three skill levels, streaming for a Medium battlefield, the core HUD, essential audio, the world map and a minimal flow.
- **Mission source:** the `MissionProvider` stub with one hand-written "dogfight" template.
- **Presentation:** explosions, smoke, wreckage, kill feedback, the debrief.
- **Prototype hygiene** (§67.5): the in-game README card (what is in, what is stubbed, known issues), a local feedback form, opt-in telemetry of the script funnel (§53.13).
- **Channel:** `alpha` opens for internal and invited testers (§66.8).
- **Explicitly not included** (§67.1): multiplayer, campaign, any generator beyond the single template, ground and naval forces, weather beyond static clear and cloudy, carriers, replay, user content, photo mode, advanced EW.

**Exit:**
1. `AC-67.1`: the script passes as an automated smoke test (headless steps 5–11 with scripted inputs) and as a human checklist, both on the Recommended profile.
2. **Gate G2:** ≥ 20 external testers complete the script; ≥ 70 % would play another sortie; median "fun" ≥ 3.8/5; the blocking-issues list is empty (panel: section 1.6).
3. **If G2 fails:** iterate on pacing, enemy behaviour and presentation before building any breadth (§67.1, risk R-05).

## 8.3 v0.3c: AI, squadrons and ground war (about 7 weeks)

**Goal:** enemies and allies behave like pilots and military units, under the same physics as the player (NN-5).

- **Pilot AI** (§19): the layered architecture, utility AI with predictive BFM, energy awareness, the **five skill levels** (Recruit, Regular, Veteran, Elite, Ace; §19.6) and the personality vector, fuel, damage, morale and retreat, weapon and target preferences, perception, navigation and avoidance, the virtual-stick interface to the FCS, radio calls (§19.13), debug visualisation (§19.14, §63.5).
- **Squadrons, basic** (§20.4–§20.8): formations, coordination, friendly AI and player orders, the GCI and AWACS controller AI, strike packages.
- **Ground warfare, basic** (§21.2, §21.3, §21.5, §21.7): unit classes and simulation tiers, SAM sites with search and track radars, basic air-defence networks, damage and destruction.
- **Naval stub** (§22, `SurfaceSim`): static ships as targets.
- **Damage** (§18.3–§18.8): the component model with cascades and effects on flight and sensors; structural overload; mid-air collisions and crashes; the aerodynamic store and damage effects (§61.5 `AeroModel`).
- **Sensors:** datalink and the shared picture (§16.5).
- **Tactical map:** tracks and threats (§55).
- **Radio v1** (§56.6) with the callsigns of Appendix D.6.
- **Factions:** the `faction/1` data model and two factions (§37.2–§37.3).

**Content:** the 16 ground unit types and the airbase installations (section 3.4); `patrol` and the merchant class as static ships; Azure League and Meridian Pact; 120 callsign words; Elite and Ace skill files.
**Exit:**
1. The AI skill ladder is monotone (`AC-59.4` precursor).
2. The same-physics audit passes (`AC-3.3`): no AI state outside the authored envelope.
3. Damage model tests (`AC-18.*`) and ground-unit tests (`AC-21.*`, the v0.3 subset) pass.

## 8.4 v0.3d: Second Playable Build (about 6 weeks)

**Goal:** the outline's second build (outline §133): multiple aircraft, better AI, more weapons, airports, take-off and landing, rearming and refuelling, objectives and reinforcements (§67.2).

- **Aircraft:** Peregrine, Swift, Bittern and Badger become flyable, each through its own conformance test (§11.19); Argus (AWACS) and Stratus (tanker) fly as AI.
- **Weapons and loadout:** eight new weapons and two stores (section 3.3); the loadout editor with computed readouts and store-dependent limits: the loadout physically changes mass, inertia, CG, drag, RCS and g-limits (§14.2–§14.6).
- **Airbase operations, core** (§48.1–§48.6): start types, the scramble system with physical engine start, taxi, take-off and form-up, base sensors and C2, the ground operations model, rearm and refuel.
- **Long-range operations** (§49.2–§49.8): sortie phases, time acceleration, cruise assistance and autopilot holds, aerial refuelling with the Stratus, range and fuel planning, return and recovery.
- **Navigation, full** (§50.4–§50.5): autopilot and flight-director modes, TACAN- and ILS-like aids, the landing assist.
- **Objectives and reinforcements:** hand-authored missions and early generator templates (Air Superiority, Strike, Interception); reinforcement waves (§28.5 basic).
- **Avionics tier B complete** (§16.10.2): tactical map, engine and fuel, countermeasures, systems and damage, navigation and targeting-pod pages. RWR and countermeasures complete.
- **Sandbox configuration:** forces, sides and AI skill (§30).
- **World:** the first two hero-polished airbases (the §67.2 base pair).

**The acceptance run (§67.2):**

| # | Step |
|---|---|
| 1 | Select an airbase pair 200 km apart |
| 2 | Pick the Peregrine with 4 × MRM + 2 × WVR |
| 3 | Launch |
| 4 | Intercept a raid detected by AWACS |
| 5 | The objective changes: a strike appears |
| 6 | Kill a SAM site |
| 7 | A reinforcement wave scrambles from the enemy base (visible taxi and take-off) |
| 8 | Land, rearm, refuel and take off again in the same session |
| 9 | Debrief with statistics |

**Exit:**
1. The run completes in ≤ 45 minutes of play with no sim errors.
2. `AC-67.2`: the automated version passes 100 % through step 9; ≥ 8 of 10 external testers complete the first six steps (panel: section 1.6).
3. Every new aircraft passes its conformance test; the balance lint (§13.2) is clean.

## 8.5 Milestone M3 exit (§66.3)

FP1 at the middle of M3 with G2 passed; FP2 at the end; hit-rate sweeps (`AC-15.5`); the AI never exceeds the authored envelope (`AC-3.3`); sensor budgets (`AC-16.*`); damage tests (`AC-18.*`). The **M3 performance review** replaces the planning allocations of §60.2 and §60.7.2 with measurements (Appendix I.6).

## 8.6 Risks

The core concept is not fun (R-05; G2 comes before any breadth); missile and sensor tuning is unvalidated (R-24; calibration in v0.3a); art throughput for four new player-grade aircraft (R-20; OQ-05 decides kit-bashing).

---

# 9. v0.4: Procedural Missions (M4, about 5 months)

`M4 · P5 (and P4) · outline §129 · about 5 months · planned` — A replayable war at any location: objectives, reinforcement logic, mission phases, friendly and enemy forces, boss encounters and dynamic events (outline §129).

## 9.0 Entry, decisions and scope

- **Entry:** v0.3 tagged; G2 passed (or its recorded substitute, section 1.6).
- **Decisions due before M5 (made during v0.4):** OQ-06 (in-game voice chat) and OQ-09 (desktop wrapper).
- **Not in v0.4:** multiplayer; naval forces, carriers and the two naval mission types (v0.5d); Chaos mode (v0.5d, it needs the governor); omniscient replays and the mission editor (v0.5e).

## 9.1 v0.4a: Mission system and generator (about 8 weeks)

**Goal:** missions are assembled from rules and data, not scripted (NN-11, ADR-14).

- **Mission system** (§25): the mission model, groups, objectives, conditions, actions and the trigger engine, phases, failure and success, the director runtime, rule sets, briefing and debrief, scoring, the file format and its provenance block.
- **Generator** (§26): its contract and request parameters, determinism and seeds (ADR-10), the pipeline, the `MissionDef` output, placement algorithms, force budgeting with threat values, validation and solvability, the **Generator Lab** tool.
- **Mission types:** the first seven (Quick Dogfight, Interception, Escort, Strike, Air Superiority, SEAD, Survival; section 3.7).
- **Sharing** (§26.3, §51): seed links, configuration sharing, mission and configuration file import and export, all validated and sandboxed (NN-6).
- **Content system** (§62): every content type in the registry; the **`NoHardcodedContentLint`** gate in CI (`AC-62.4`).
- **Tools:** mission and generator panels (§63).

**Exit:**
1. 10⁴-seed batch validity ≥ 99.5 % (`AC-26.*`, §66.3) and solvability 100 % on the batch.
2. Generated missions hash equal on WASM and native.
3. `NoHardcodedContentLint` clean.

## 9.2 v0.4b: Escalation, bosses, anti-repetition and a living world (about 7 weeks)

**Goal:** missions play like stories with rising and falling tension, and no two are alike (§28, §69).

- **Escalation** (§28): the phase pattern, the intensity director, event types, the reinforcement director, objective changes and chaining, crisis design, elite response and cinematic cues, resolution and extraction, fairness rails.
- **Bosses and aces, single session** (§29.1–§29.4, §29.8): the capability sheet, boss data, encounter design and tuning; no health multipliers (NN-4).
- **Anti-repetition, core** (§69.1–§69.7): mission DNA, history and penalties, the **17 twists**, player freedom, cinematic moments, variation axes.
- **Squadrons** (§20.2, §20.3, §20.9): squadron definitions, doctrine, callsigns, identity.
- **Ground warfare** (§21.4, §21.5, §21.8, §47.6): IADS with command nodes, convoys and logistics on the road graph, placement hooks, targets from infrastructure.
- **Weather, full** (§24.2–§24.4, §24.7, §46.4): eight presets, weather fields and fronts, dynamic changes as mission events, weather effects on visibility, radar and handling, seasons; wind, turbulence and orographic effects in the flight model (§11.4); the `WeatherProvider` seam's real implementation.
- **Civilian world** (§23.2–§23.3): analytic ambient traffic and civilian air corridors.
- **Line of sight** through clouds and buildings (§61.5).
- **Scale:** the reduced-order flight tiers T1–T3 for distant aircraft (§11.17) and simulation priority for Large battles (§60.7).
- **EW additions:** the self-protection ECM pod, decoys, basic ESM for SEAD (§17.5–§17.7).
- **Mission types:** seven more (Suppression, Convoy Attack, Reconnaissance, Bombing, Mixed Strike, Boss Hunt, Dynamic Frontline), for 14.
- **Factions:** Concord of Free Ports and Iron Marches; doctrines in force budgeting; procedural emblems and liveries with the similarity lint (§37.4); the era filter and the Kestrel G3 variant.

**Content:** Condor and Albatross (flyable), Scout (flyable remotely and as AI), Hauler (AI); seven weapons and two stores (section 3.3); nine ground unit types and the infrastructure targets (section 3.4); three more hero airbases (five in all); four more regions (ten in all).
**Exit:**
1. `AC-3.4`: mean DNA distance ≥ 0.45; SDI ≥ 3 for primary objectives.
2. Escalation tests (`AC-28.*`) and boss tests (`AC-29.*`) pass.
3. **Gate G3:** paired sessions at the same place with different seeds rated "substantially different" by ≥ 80 % (panel: section 1.6); no seed in 10⁴ yields an unwinnable setup.
4. PS-1 (75 aircraft, rugged terrain, clouds, 20 minutes on Recommended: average frame ≤ 16.7 ms, 1 % low ≥ 45 FPS, sim ≤ 4.0 ms).

## 9.3 v0.4c: Player-facing systems (about 5 weeks)

**Goal:** the sandbox is a complete single-player product: configurable, learnable, accessible, recordable.

- **Difficulty and realism** (§59): all dials, the five presets complete, the Realism Index and scoring.
- **Accessibility baseline** (§57.2–§57.7): input and motor, visual, auditory and cognitive options, browser specifics. **G7 baseline.**
- **UI flow, full** (§53): briefing, the loadout screen, debrief, contextual hints, the encyclopaedia, the loading experience, telemetry and privacy.
- **Training** (§53.11): First Flight and training missions 1–8.
- **Replay and photo mode v1** (§52.2–§52.8): SSR files, seeking, instant replay and kill cam, photo mode; the `ReplaySink` seam writes files.
- **Statistics and logbook** (§65).
- **Sandbox configuration complete** (§30), except Chaos mode.
- **1v1 against the AI** on one machine with the ten curated arenas (§31.8).
- **Avionics tier A** for Easy (§16.10.1); the tactical map's full information model (§55).
- **Head tracking** (§58.7).
- **Audio, full mix** (§56.2–§56.7): propagation, spatial audio, the worklet backend, radio personas; the music director with placeholder stems (§56.8). **G4 audio gate.**
- **Customisation** (§14.7–§14.8): liveries, insignia and callsign; loadout presets and sharing.

**Exit:**
1. G7 baseline (§57.9).
2. ET-1, the ideal sandbox session (§70.2), passes in its quantitative parts.
3. Usability: SUS ≥ 75 (or its proxy, section 1.6); first-flight completion (`AC-53.7`).
4. `AC-3.1` ≥ 99 % (§66.3 sets the M4 target).
5. G4 audio: aircraft sound distinctiveness (`AC-56.7`).
6. PS-7: a 2-hour mixed mission with memory growth < 1 % per hour and no worker restarts.

## 9.4 Milestone M4 exit (§66.3)

10⁴-seed validity ≥ 99.5 %; diversity (`AC-3.4`); solvability; escalation tests (`AC-28.*`); **G3**. This roadmap adds `AC-3.1` ≥ 99 % (from §66.3 M2) and the G7 baseline (from §66.4).

## 9.5 Risks

Unsolvable or repetitive missions (mitigation: solvability validation and DNA plots in CI); generator performance on Large battles (§26.10); scope: v0.4b is the largest sub-milestone in the plan (if it slips, the twists and the civilian world are the first to move to v0.4c).

---

# 10. v0.5: Multiplayer (M5, about 5 months)

`M5 · P6, P7, P8 · outline §130, §134 · about 5 months · planned` — A large cooperative air operation over LAN/VLAN, with no internet required (NN-8), plus competitive 1v1; then the Third Playable Build and the public beta.

## 10.0 Entry, decisions and scope

- **Entry:** v0.4 tagged; OQ-06 (voice chat) and OQ-09 (desktop wrapper) decided.
- **Decision due before the public beta (v0.5e):** OQ-08 (content rating and map and border depiction).
- **Not in v0.5:** the campaign (v0.6), co-op campaign (M7), the community hub (M7), submarines (M7).

## 10.1 v0.5a: Transport and replication (about 5 weeks)

- **Topology and transport** (§33.2–§33.3): host and client roles; WebRTC data channels first, WebSocket as the LAN fallback, WebTransport where available; the handshake and the shared clock; serverless invite blobs and QR codes (§32.2); the `Transport` seam's real implementation.
- **Server-authoritative simulation** (ADR-08) with snapshot replication, relevance filtering and the priority accumulator (§33.5).
- **Robustness** (§33.7, §33.8, §33.11): connection quality and failure handling, offline and LAN operation, the LAN/VLAN trust model.
- **Tools** (§63.6, §33.12): the network debug panel and the impairment simulator.

**Exit:**
1. A two-client session stays stable across the impairment matrix (§33.13).
2. A LAN session works with the internet disconnected.

## 10.2 v0.5b: Prediction and hosting (about 5 weeks)

- **Prediction** (§33.6): client prediction, reconciliation, interpolation, lag compensation.
- **Hosting** (§33.9, §33.10): the listen server in a browser tab; the native dedicated-server build and its container.
- **Joining** (§33.4, §32.8): join and state transfer, tile relay by the host, leave and drop-out handling.
- **Voice** (§32.9, §56.12): in-game voice per lobby, as OQ-06 decides.
- **Budgets** (§60.11): network performance.

**Exit:**
1. Hit registration meets §33.14 at 80 ms RTT and 2 % loss.
2. The 16-client headless soak passes (`AC-33.*`).

## 10.3 v0.5c: Co-op, 1v1 and roles (about 5 weeks)

- **Co-op** (§32.3–§32.7, §32.10): the lobby and host settings, the seven roles (fighter, interceptor, strike, bomber, electronic warfare, reconnaissance, support) with their role UIs, scaling with player count, **multi-crew** seats (Condor WSO, Magpie EWO, Argus ABM), non-voice communication, co-op rules and flow. The generator receives `roles_present` and gives every role meaningful tasks.
- **1v1 online** (§31.2–§31.10): flow, setup, rule sets, boundaries and adjudication, fairness, spectating, the round state machine.
- **EW role** (§17.8): the EW console and the stand-off jammer pod.
- **Multiplayer rules elsewhere:** boss scaling (§29.7), difficulty locks (§59.3), the shared tactical picture (§55.9).

**Content:** Magpie and Argus become flyable; the `ecm3` pod; tanker controls for the support role.
**Exit:**
1. Co-op tests (`AC-32.*`) and the 1v1 fair-match audit (`AC-31.*`, §68.1 P8) pass.
2. A four-player session with four different roles gives each role tasks and scores its contribution.

## 10.4 v0.5d: Forces at scale: ground, naval and carriers (about 5 weeks)

**Goal:** the Third Playable's ground forces, naval forces and larger battles (outline §134).

- **Naval warfare** (§22.1–§22.6, §22.8–§22.9): ship classes, movement and navigation, ship sensors and defence, damage and sinking, task groups and the fleet datalink; the `SurfaceSim` seam's real implementation.
- **Carriers** (§22.6, §10.2, §48.2): the carrier as a mobile airport; catapult launch and arrested landing as forces on the aircraft (§11.13); wind over deck; deck motion and its limits; the carrier AI's deck cycles; the carrier start type.
- **Ground forces at scale** (§21.3, §21.4, §21.6): ground combat among AI, pathfinding at battle scale, simulation tiers.
- **Civilian world** (§23.4–§23.5): civilian ships, road and rail traffic.
- **Scale** (§60.7.3, §30.4): the Performance Governor, Huge battles and Chaos mode.
- **Replication:** the wind field (§24.9).
- **Mission types:** Carrier Defense and Fleet Battle, for all 16.

**Content:** the Gannet; `carrier_large`, `cruiser`, `destroyer`, `frigate`; Skua-1, Tern-2, TT-1, Lance-6; the towed decoy `td1`.
**Exit:**
1. `AC-22.1`: catapult end speed within 1 % of target; the reference case covers 94 m in 2.2 s ± 0.1 s; the rest of `AC-22.*`.
2. PS-2 (150 aircraft on the High profile: 60 FPS average, sim ≤ 4.5 ms, the governor at step ≤ 5) and PS-5 (carrier operations with 50 ships at a high sea state: ≥ 55 FPS).
3. The bandwidth budget holds (`AC-60.8`).

## 10.5 v0.5e: Third Playable Build and the public beta (about 2 weeks, plus content in parallel)

- **Regions:** at least 20 pre-packaged regions (outline §134).
- **Replay** (§52.4–§52.6): omniscient replays (OSR), the replay viewer, multiplayer replay rules.
- **User content v1** (§51.3, §51.6, §51.7): the mission editor, user-defined battlefields, location tools.
- **Avionics tier C** (§16.10, §54.10): the complete page set per aircraft.
- **Accessibility complete** (§57) and the first external accessibility review (§57.8).
- **Localisation** (§53.10, §56.9): UI text and subtitles in the eight target languages.
- **Content:** Wraith, Chimera, Pike-9, Dirk-C (✂: may move to v0.7a); ten hero airbases in all.

**The acceptance run (§67.3):** 4 human clients, a dedicated server or host, and 70 AI on a Large battlefield in a new region; ground-convoy and naval task-group objectives; air superiority, strike and EW roles; escalation to the Climax phase; a boss encounter; all clients finish with consistent results; replays saved; network budgets met (`AC-60.8`); no desync alert in 30 minutes; frame-time gates on Recommended.

**The public beta** (§66.8): the opt-in `beta` channel opens with opt-in telemetry. Its content is the "first shippable product" of §1.4: Sandbox Skirmish, 1v1 and LAN/VLAN co-op, 12 aircraft, 26 weapons and 6 stores, 4 factions, worldwide streaming with at least 20 packaged regions.

**Exit:**
1. **Gate G5:** the §33.14 criteria; the 16-client soak; LAN-only play verified (`AC-61.8`); the invite flow succeeds on ≥ 95 % of 20 home networks (panel: section 1.6).
2. `AC-67.3`: the 30-minute four-human soak with no desync and p99 frame time within budget.
3. ET-3, the multiplayer experience (§70.4).
4. G7 complete for M5 (§66.4).

## 10.6 Milestone M5 exit (§66.3)

The 16-client soak (`AC-33.*`), hit registration under impairment (§33.14), bandwidth within budget (`AC-60.8`), **G5**, LAN-only play without internet (`AC-61.8`).

## 10.7 Risks

Browser networking limits (mitigation: invite blobs, the WebSocket LAN fallback, the impairment matrix); desync under load (state hashes per tick in debug sessions); the content volume of v0.5e (the ✂ items move to v0.7a first).

---

# 11. v0.6: Campaign (M6, about 6 months)

`M6 · P9 · outline §131, §135 · about 6 months · planned` — Airports become strategic assets in a persistent roguelite war (§34). The campaign is a strategic layer over the same mission architecture (ADR-14), not a separate game.

## 11.0 Entry and scope

- **Entry:** v0.5 tagged; the public beta running.
- **Not in v0.6:** co-op campaign (§32.11, staged M6+; M7 here).

## 11.1 v0.6a: The strategic layer (about 7 weeks)

- **Campaign structure** (§34.2–§34.7): structure, generation, the roguelite run, operations that call the same generator, the architecture hooks filled in (`IForceProvider`, `IAirframeSource`, `PersistenceSink`), seeds and reproducibility.
- **Strategic map** (§35.1–§35.5): the airport graph, node state, frontlines, reach and legal operations, the strategic AI's turn.
- **Factions in the campaign** (§37.5–§37.7): the living enemy roster, relations, communication style.
- **Save system complete** (§64.3–§64.7): storage layout and transactions, autosave, checkpoints and the emergency checkpoint, versioning, migration and integrity, eviction, export and import.

**Exit:**
1. A run starts, plays an operation, saves and resumes; the kill-the-tab test (`AC-64.1`) and the rest of `AC-64.*`.
2. `AC-6.5`: the campaign can be compiled out without changing Sim Core code.

## 11.2 v0.6b: Capture, damage, resources and events (about 6 weeks)

- **Airport capture** (§36): capture difficulty, the capture meter, after capture, counter-capture.
- **Persistent airbase damage and repair** (§48.7) and the dynamic frontline (§48.9).
- **Resources** (§39): the resources, income and costs, aircraft states and losses, supply logistics, the readiness screen.
- **Events and decisions** (§38.4, §40): the event deck (about 24 entries, section 3.8), scheduling, coupling events to operations, strategic decisions.
- **The world at war:** wartime state of cities (§47.9) and campaign effects on civilian traffic (§23).

**Exit:**
1. A capture changes reach and supply.
2. Sortie rates follow facility health (`AC-48.4`).
3. Capture, resource and event tests (`AC-36.*`, `AC-39.*`, `AC-40.*`).

## 11.3 v0.6c: People and knowledge (about 6 weeks)

- **Pilot progression** (§41): the model, traits, survivability and injuries, the roster.
- **Fog of war and intelligence** (§42): the knowledge model, sources of intelligence (including the Scout and the reconnaissance pod), identification progression, in-mission fog and briefings, fog on the strategic map.
- **Emergent storytelling** (§43): the event stream, storylets, the chronicle, relationship memory and callbacks.
- **Persistence of identity:** bosses and aces (§29.5–§29.6), squadrons (§20.9), campaign anti-repetition (§69.8).
- **Career and campaign statistics** (§65.6).

**Exit:**
1. The chronicle test (§43.7).
2. A recurring ace reappears within five operations in ≥ 70 % of automated runs.
3. Pilot and intelligence tests (`AC-41.*`, `AC-42.*`).

## 11.4 v0.6d: Campaign Build (about 5 weeks)

- **Progression** (§38.3, §38.5–§38.7): enemy adaptation, Legacy Points meta-progression, campaign difficulty, run end and recap.
- **Campaign UI** (§35.6): the campaign map layers, the run summary.
- **World:** fifteen hero airbases in all.

**The acceptance run (§67.4):** start a run with seed S; one airport owned, neighbours hostile; pick an operation; win; capture the airport and see the map change; the enemy counterattacks a base; an ace squadron appears, survives and reappears two operations later; persistent damage limits sortie rates; resources constrain loadouts; progress to a strategic victory; run summary and Legacy Points; plus the automated AI-against-AI soak of 20 runs × 3 seeds.

**Exit:**
1. **Gate G6:** ≥ 60 % of testers continue past their third operation (panel: section 1.6); a campaign takes 6–15 hours on Normal; zero soft-locks in the soak.
2. `AC-67.4`: 60 runs, no soft-locks, ≥ 95 % reaching victory or defeat within the expected number of operations.
3. ET-2, the campaign experience (§70.3).

## 11.5 Milestone M6 exit (§66.3)

The Campaign Build (§67.4); the 20-run soak; save and migration tests (`AC-64.*`); **G6**.

## 11.6 Risks

Campaign soft-locks (mitigation: the AI-played soak on every build); browser storage eviction (ADR-19: persistence requests, export and import); the length of a run (tuned against the 6–15 hour target).

---

# 12. v0.7: Content & Polish (about 3 months)

`M6 exit work · G4, G7, G8 · about 3 months · planned` — Fill out and finish everything that is data or presentation, without changing the engine.

## 12.1 v0.7a: Content completion (about 6 weeks)

- **Content:** `carrier_light`; the Dust weather preset; the release set of regions; at least 30 hero-polished airbases; anything cut to here from v0.5e (Wraith, Chimera, Pike-9, Dirk-C).
- **Audio:** full voice banks per faction persona; the final music (licences in G8).
- **Art:** the polish pass over every aircraft and cockpit against G4 before the content drop.
- **Balance:** the §13.2 balance bands, tuned with the beta's telemetry.

**Exit:**
1. Content gates pass (balance bands §13.2, `AC-56.7`); `NoHardcodedContentLint` clean.
2. The content row of §71.6 is complete: ≥ 12 aircraft, 26 weapons + 6 stores, 4 factions, ≥ 20 regions + worldwide streaming, 16 mission types, ≥ 30 hero airbases.

## 12.2 v0.7b: Accessibility, localisation and UX (about 6 weeks)

- **Accessibility:** the second external review (§57.8), required for 1.0 by §71.6.
- **Localisation:** the strings added since v0.5e, in the final language list.
- **Replay and cinematics** (§52.8–§52.10): photo mode, video capture and the cinematic director complete.

**Exit:**
1. **Gate G7** complete, with the external review done.
2. Usability SUS ≥ 75; first flight in ≤ 10 minutes (`AC-4.5`).

---

# 13. v0.8: Beta Hardening (about 2 months)

`G5/G6 re-runs · about 2 months · planned` — The full game, campaign included, in the public beta.

## 13.1 v0.8a: The campaign in the beta (about 4 weeks)

- The campaign joins the `beta` channel; opt-in telemetry (§53.13, §66.8).
- Crash-free session tracking; the distribution of performance profiles.
- **Gate G8 (licence and IP):** the descriptor audit (`AC-45.4`), the naming and IP review of every fictional name, emblem and callsign (ADR-11, §37.4), music and audio licences, the privacy statement.

## 13.2 v0.8b: Beta fixes (about 4 weeks)

- G5 and G6 re-run on real home networks and a wider hardware set.
- Balance and tuning from telemetry and playtests; the fix list burned down.

**Exit:** crash-free sessions ≥ 99.5 %; no open P0 or P1 bugs; G8 passed.

---

# 14. v0.9: Release Candidate (about 1 month)

`G9 preparation · about 1 month · planned`

## 14.1 v0.9a: Freeze and verify

- Feature freeze; bug fixes only.
- Full regression: every acceptance criterion, the performance gates PS-1 to PS-9 on the three profiles, the screenshot suite, the security and CSP audit (`AC-61.6`), the reproducible-build check.
- Platform: the compatibility matrix published (Chrome and Edge primary; the Firefox and Safari status stated); offline, PWA install, CSP and COOP/COEP verified; the decision on the WebGL2 fallback (§66.6.5).
- Documentation: the player manual, controls cheat sheets, UGC and modding documentation, the support runbook.

**Exit:** the §71.6 release checklist is all green.

---

# 15. v1.0: Release

`G9 · §151–§153` — Version 1.0 is M6 complete (§71.6).

| Area | Criterion (§71.6) |
|---|---|
| Experience | ET-1, ET-2 and ET-3 pass |
| Success areas | All eleven rows of §71.2 meet their thresholds, each with an owner and evidence (`AC-71.1`) |
| Quality | No open P0/P1 bugs; crash-free sessions ≥ 99.5 % in the beta; performance gates on the three profiles |
| Content | ≥ 12 aircraft, 26 weapons + 6 stores, 4 factions, ≥ 20 pre-packaged regions + worldwide streaming, 16 mission types, ≥ 30 hero-polished airbases |
| Platform | Compatibility matrix, offline, PWA install, CSP and COOP/COEP verified |
| Legal | Data licence audit green (`AC-45.4`); fictional-content IP review; music and audio licences; privacy statement |
| Accessibility | G7 complete; external review done |
| Documentation | Player manual, controls cheat sheets, modding and UGC docs, support runbook |

- The `stable` channel is published; `datapack.json` and the content packs are live on the CDN.
- Post-launch indicators (§71.7) are watched to inform tuning, not as design goals.
- The long-term track begins.

---

# 16. LONG-TERM TRACK (M7)

`P10 · outline §145 · ongoing` — Release trains after 1.0 (§66.7). Each train passes G7–G9 and is data-only wherever possible (hot-updated through the Service Worker).

| Train | Content and capability | Enabled by |
|---|---|---|
| **Aircraft and weapons** | Roster growth from 12 towards 30; era packs (G3 to G5); further anti-ship, cruise and glide weapons beyond the launch set; a directed-energy research item | §62 data packs |
| **Factions** | More fictional factions and doctrine variants; faction-specific campaigns | §37, §62 |
| **Regions and data** | More pre-packaged regions; better building coverage; seasonal styles without imagery; polar support (§45.8) | §45 |
| **Large-scale battles** | Higher caps (WASM threads, GPU culling); a 250+ aircraft entertainment mode | §60 |
| **Naval and submarines** | `sub_1`, anti-submarine warfare, port operations; training mission 9 (carrier ops) | §22.7, §53.11 |
| **Air defence** | IADS with command and control, mobile SAM tactics, passive sensors | §21 |
| **Electronic warfare** | Advanced EW: ESM trees, deception, seeker interference, dedicated EW multiplayer play | §17 |
| **Campaign** | New operations, strategic modifiers, narrative arcs, daily and weekly challenge seeds; **co-op campaign** (§32.11) | §34–§43 |
| **Multiplayer** | 32 humans with a dedicated server, spectator mode, tournament tooling | §33 |
| **User content** | The community hub (OQ-10); signed custom aircraft packs; DSL scripting expansion | §51, §62 |
| **Replay and cinematics** | More camera tools, an editing timeline, export presets | §52 |
| **Cockpit and controls** | The clickable cockpit (§16.10.3); force feedback and haptics (§58.9) | §16, §58 |
| **Platform** | WebXR (OQ-07), a desktop wrapper (OQ-09), mobile and tablet research | §58.8 |

---

# 17. CROSS-CUTTING TRACKS

## 17.1 Performance scenarios (§60.9) by version

Scenarios become gates in the version where their content first exists (§60.9: "introduced as their scenarios become possible"). Each gates ADR names the reference machine (ADR-28).

| Scenario | Content | First gated in |
|---|---|---|
| PS-0 (project) | The arena frame and control-latency gates (ADR-28, ADR-33, ADR-39) | ✅ v0.0b, extended in v0.1b and v0.1c |
| PS-9 Microbench | Per-system benchmarks, no regression > 5 % | As each system appears: flight tiers v0.1, tile decode and mesh build v0.2, LOS and DLZ lookup v0.3, the netcodec v0.5 |
| PS-4 Dense city | 40 k buildings, night | v0.2c |
| PS-3 Cold streaming | Mach 1.0 at 150 m over mountains, cold cache | v0.2d |
| PS-6 Minimum profile | Medium battle, Low preset, 900p | v0.2d (flight only), v0.3b (with the duel), v0.4c (Medium battle) |
| PS-8 Load time | Cold and cached loads | v0.2d |
| PS-7 Soak | 30 minutes at v0.1d; 2 hours from v0.4c | v0.1d / v0.4c |
| PS-1 Large battle | 75 aircraft, clouds, 20 minutes | v0.4b |
| PS-2 Huge battle | 150 aircraft, High profile | v0.5d |
| PS-5 Carrier + naval | Carrier ops, 50 ships, high sea state | v0.5d |

## 17.2 Determinism and goldens

The state-hash and golden policy (ADR-23) applies to every version. New determinism suites arrive with their systems: the combat suite (v0.1d), streamed flight (v0.2b), generated missions (v0.4a), multiplayer sessions with per-tick hashes in debug builds (v0.5a), campaign runs (v0.6a). Re-blessing a golden is always a recorded decision with a proof of which columns moved (the v0.1d pattern, `cargo xtask golden-diff`).

## 17.3 Playtests

The GDD's programme (§66.4): weekly internal sessions from M1, monthly external sessions from M2, a structured protocol (script, think-aloud, SUS, consented telemetry). In a solo project the owner's sessions are the internal programme; external sessions depend on section 1.6.

## 17.4 Accessibility

Baseline settings ✅ v0.1c (comfort, photosensitivity notice, ADR-44) → G7 baseline v0.4c → complete and the first external review v0.5e → the second external review v0.7b. Every UI change is checked against §57.8 in CI from v0.4c.

## 17.5 Geodata and licensing

The licence audit runs in every pack build from v0.2a (§45.9, Appendix H); G8 runs before each datapack and content release; OQ-04 decides which obligations apply.

## 17.6 Art, audio and localisation

Sections 3.10, 3.11 and 3.12.

## 17.7 Security and privacy

The content sandbox for shared missions (NN-6) from v0.4a; the LAN/VLAN trust model (§33.11) at v0.5a; telemetry consent (§53.13) from the alpha; the CSP audit (`AC-61.6`) at v0.5e and v0.9a.

## 17.8 Documentation

A gates ADR before each sub-milestone; an exit record after it (`docs/exit/`); an ADR for every decision that changes the GDD; the GDD amendments of section 19 through Appendix I.4; player-facing documentation at v0.9a.

## 17.9 Release channels (§66.8)

`nightly` on every merge from v0.0; `alpha` from v0.3b; `beta` from v0.5e; `stable` from v1.0. The multiplayer protocol version is pinned per release train (§62.7).

---

# 18. DECISIONS DUE, CUT LINES AND RISKS

## 18.1 Open questions by deadline

| ID | Item | GDD deadline | Needed by (this roadmap) | Status |
|---|---|---|---|---|
| OQ-01 | Stack: Rust → WASM sim; renderer | Before M1 | — | Closed (ADR-02 stands; ADR-40) |
| OQ-02 | Team size and release scale | Before M1 planning | **Now** (it changes every duration) | Closed 2026-11-12: solo (ADR-50) |
| OQ-03 | Hosting, CDN, tile bandwidth | Before M2 | **Before v0.2a** | Decided for M2 2026-11-12: the app on GitHub Pages, the GeoPacks on Cloudflare R2 (ADR-50) |
| OQ-04 | Commercial or free | Before M2 | Before v0.5e (moved by the owner) | Open; all rights reserved; commercial-safe data only until decided (ADR-50) |
| OQ-05 | Art budget: kit-bashed or bespoke | Before M3 | During v0.2 (the hero Kestrel, v0.2c) | Open |
| OQ-07 | WebXR stance | Before M3 | During v0.2 | Open |
| OQ-12 | Human panels for G2, G3, G5, G6, G7 and the §67.2 run (section 1.6) | — | Before v0.3b | Closed 2026-11-12: automated proxies decide, the owner's runs are recorded (ADR-50) |
| OQ-06 | In-game voice chat | Before M5 | During v0.4 | Open; assumed on |
| OQ-09 | Desktop wrapper | Before M5 | During v0.4 | Open |
| OQ-08 | Content rating; map and border depiction | Before public release | **Before v0.5e** (the beta is public) | Open |
| — | Final list of target languages (Appendix I.7) | — | Before v0.5e | Open |
| OQ-10 | Community hub and moderation | Before M7 | v0.9 | Open |
| OQ-11 | G1(a) blind panel | Not set | Not set (the owner decides) | Open |

## 18.2 Owner decisions this roadmap needs

1. ✅ Accept or change the rows of section 19 (each row says which GDD text would be amended). *Accepted in full, 2026-11-12; GDD 0.2.5.*
2. ✅ Decide OQ-03 and OQ-04 before v0.2a, and OQ-02 when possible. *2026-11-12: R2; OQ-04 kept open with commercial-safe data; solo.*
3. ✅ Choose the six pilot regions (proposal in section 3.9). *The proposal, 2026-11-12.*
4. ✅ Decide how G2 is run (section 1.6 recommends a real panel). *Automated proxies and the owner's sorties (OQ-12), 2026-11-12.*

All four are recorded in [ADR-50](adr/0050-entering-m2.md). The next owner decisions are OQ-05 and OQ-07, during v0.2.

## 18.3 Cut lines

If the schedule slips, cut from the bottom of the priority list (§66.6, §68.2.4). A cut system keeps its interface and its stub; nothing integrated is removed.

1. **Advanced systems** (`P10`: deep naval, logistics detail, EW depth, civilian systems) move to M7.
2. **The beta may ship 8 aircraft and 12 weapons** (§66.6.2, applied to the beta per R20). The first items to move to v0.7a are Wraith, Chimera, Pike-9 and Dirk-C (marked ✂).
3. **v0.5 may ship without WebTransport or voice** (§66.6.3); WebRTC data channels and the WebSocket LAN fallback stay.
4. **v0.6 may ship two factions and no storylets** (§66.6.4); the event stream stays.
5. **The WebGL2 fallback may be dropped** if WebGPU coverage is sufficient at release (§66.6.5); decided at v0.9a.
6. **v0.4b's twists and civilian world** move to v0.4c if v0.4b slips.
7. **`carrier_light`** moves to M7.
8. Not cuttable without changing the release criteria (§71.6): 16 mission types, ≥ 20 regions, ≥ 30 hero airbases, 4 factions.

## 18.4 Risks by version

The full register is Appendix F of the GDD.

| Version | Main risk | Mitigation |
|---|---|---|
| v0.0 | Cross-origin isolation or WASM threads unavailable on the host | The degraded single-worker mode, designed in from the start (§61.2, ADR-17) |
| v0.1 | Flight feel not good enough | G1 blocks content breadth; automated feel proxies (ADR-45) |
| v0.2 | Streaming holes or hitches; data licensing; tile bandwidth | Residency invariant and sim terrain layer (§44.5, §44.6); the licence audit; OQ-03 and OQ-04 |
| v0.3 | The core concept is not fun (R-05); missile tuning unvalidated (R-24); art throughput (R-20) | G2 at v0.3b before any breadth (decided by proxies and the owner's sorties, OQ-12); calibration in v0.3a; OQ-05 |
| v0.4 | Unsolvable or repetitive missions; v0.4b's size | Solvability validation and DNA metrics in CI; the v0.4b cut line |
| v0.5 | Browser networking limits; the content volume of the beta | Invite blobs, the WebSocket fallback, the impairment matrix; the ✂ items |
| v0.6 | Campaign soft-locks; storage eviction | The AI-played soak on every build; persistence requests, export and import (ADR-19) |
| v0.7–v1.0 | Scope and licensing | Cut lines; G8 |
| All | Human-panel gates in a solo project | Section 1.6; OQ-12 |

---

# 19. RECONCILIATION WITH THE GDD AND THE OUTLINE

Each row is a place where the sources disagreed. "Decision" is what this roadmap schedules; "GDD amendment" is the text that should change through Appendix I.4 once the owner accepts the row. Rows without an amendment fix the old roadmap only.

**Accepted in full on 2026-11-12** ([ADR-50](adr/0050-entering-m2.md)). The amendments are in GDD 0.2.5. R23's was already in 0.2.4. Some passages next to an amended one were also made consistent with it:
- §66.3 M3 now reads "navigation (autopilot modes, aids)" and "tactical map (tracks and threats)";
- M4 gains civilian traffic;
- M5 gains the carriers and the last two mission types;
- §61.5 dates the first damage, missile and pilot implementations to v0.1d.

| # | Topic | What the sources say | Decision in this roadmap | GDD amendment |
|---|---|---|---|---|
| R1 | **Naval forces and carriers** | §22's tag: "M4 (basic ships and carriers) → M6+ (submarines)". §61.5 `SurfaceSim`: static ships M3, moving ships and carriers M5. §66.3 M5: "ground/naval forces (§21–§22)". §67.1 excludes carriers from FP1; §67.3 adds naval forces at FP3; outline §134 likewise. Old roadmap: v0.5c | Static ships v0.3c; moving ships, task groups and **carriers v0.5d**; `carrier_light` v0.7a; submarines M7 | §22 tag → "M3 (static ships) → M5 (ships, carriers) → M7 (submarines)" |
| R2 | **Mission types at M4** | §66.3 M4: "§27 mission types (16)". §67.3 lists "16 mission types" among FP3's additions at M5. Carrier Defense and Fleet Battle need R1's ships. Old roadmap: 8 at v0.4a, the rest at v0.4b, and "all 16" again at v0.7a | 7 at v0.4a, **14 by v0.4b**, **16 at v0.5d**; removed from v0.7a | §66.3 M4 → "(14; Carrier Defense and Fleet Battle at M5)" |
| R3 | **Combat model fidelity at M3** | §61.5 puts the real damage, radar, missile and AI-pilot models and full LOS at M4. §66.3 M3 lists the component damage model, radar with notching, the DLZ baker, utility + predictive BFM and max-mip LOS. §67.1 (FP1) uses aggregate damage, simplified radar modes and a single bandit; §67.2 (FP2) adds "better AI (utility + BFM)" | Missiles, the radar model and max-mip LOS at **v0.3a** (FP1 uses simplified radar modes on it); aggregate damage for FP1; component damage and utility BFM at **v0.3c**; LOS through clouds and buildings at v0.4b | §61.5: final-implementation milestone M3 for `DamageModel`, `RadarModel`, `MissileModel`, `PilotBrain`; `LosService` M3 (max-mip) / M4 (clouds, buildings) |
| R4 | **Navigation and long-range operations** | §50's tag: `P2 · M2`; §49's: `M2–M4`. §66.3 lists both under M3. Old roadmap: v0.3d. The two-location test (M2) spans up to hundreds of kilometres | Basic navigation (waypoints, bearing and distance, holds) **v0.2d**; autopilot modes, aids, time acceleration, AAR v0.3d; intercept in transit v0.4b | §66.3 M2: add "§50 navigation (basic)" |
| R5 | **Tactical map** | §55's tag: "M2 (basic) → M4 (full)". §66.3 lists §55 under M3. Old roadmap: v0.3c | Basic **v0.2d**; tracks and threats v0.3c; full v0.4c | §66.3 M2: add "§55 tactical map (basic)" |
| R6 | **Civilian world** | §23's tag: `P10 · M4–M6`. §66.3 M3: "§23 civilian traffic (basic)". Old roadmap: absent | Ambient traffic and air corridors **v0.4b** (the `civilian_airliner` twist needs them); ships, road and rail v0.5d; campaign effects v0.6b. Reason: `P10`, and nothing in M3's builds needs it (§68.2.1) | §66.3 M3: move §23 to M4 |
| R7 | **Weather** | §24's tag: "M2 (sky, time, wind) → M4 (full, dynamic)"; §61.5 agrees. §66.3 M4 does not list §24. Old roadmap: only the basic version (v0.2c) | Basic v0.2c; **full v0.4b** | §66.3 M4: add "§24 full weather" |
| R8 | **Worldwide streaming** | §66.3 M2's exit includes `AC-3.1` (1 000 random locations), which needs the whole Earth. Old roadmap: "regions up to the release set plus worldwide streaming" at v0.7a | Worldwide from **v0.2a**; 6 → 10 → ≥ 20 packaged regions (v0.5e); v0.7a only completes the release set | — |
| R9 | **Training missions** | §53.11: ordinary `MissionDef`s; §53's tag: full flow at M4. Old roadmap: v0.7b | **v0.4c**; carrier ops M7 | — |
| R10 | **Accessibility** | §57's tag: "M1 (baseline) → M5 (complete)". §66.4 G7: "M4 baseline, M5 full, M7 external review". §57.8: external review before M5 and M7. §71.6: 1.0 needs the external review done. Old roadmap: full at v0.7b | Baseline v0.4c; **complete and first review v0.5e**; second review **v0.7b** (before 1.0) | §66.4 G7 and §57.8: "M7 external review" → "external review before 1.0" |
| R11 | **Localisation** | §53.10: target languages by M5; §56.9: subtitles from M5. Old roadmap: v0.7b | **v0.5e** | — |
| R12 | **Release channels and the first product** | §66.8: `alpha` from M3, `beta` from M5. §1.4: the first shippable product has no campaign ("a later content release"). §71.6: 1.0 = M6 complete. Old roadmap: the beta at v0.8 | `alpha` v0.3b; **public beta v0.5e** with the §1.4 product; the campaign joins the beta at v0.8a; 1.0 includes the campaign | §1.4: "first shippable product" → "first public (beta) product; the campaign is part of 1.0 (§71.6)" |
| R13 | **Ships at 1.0** | Appendix D.1 counts 8 ship classes at 1.0, which includes `sub_1`; D.4, §22.7 and §66.7 stage the submarine after launch | **7 classes at 1.0**; the submarine in M7 | D.1: "8" → "7 (the submarine post-launch)" |
| R14 | **MFD pages** | §54's tag: "M5 (MFD page set)". §66.3 M3: "§54 HUD (DLZ, RWR, radar page)". §16.10 defines tiers A, B, C. Old roadmap: only v0.3a's pages | Tier B v0.3a and v0.3d; tier A v0.4c; **tier C v0.5e** | — |
| R15 | **Replay** | §52's tag: `P5 · M5`. §66.3 M4: "replay (SSR) and photo mode v1". §61.5: ring buffer M2 → files M5. §6.1: replay viewer M5+ | Ring buffer v0.2d; SSR and photo mode v1 **v0.4c**; OSR and the viewer v0.5e; video and the cinematic director v0.7b | §52 tag → "M4 (SSR, photo v1) → M5 (OSR, viewer)" |
| R16 | **User content** | §51's tag: "M4 (seed/config sharing, file import/export) → M5 (editor) → M7 (hub)". Old roadmap: no file import/export | File import/export added to **v0.4a** | — |
| R17 | **Squadrons** | §20's tag: `P4 · M4`. §66.3 M3: "§20 squadrons/formations/wingman orders"; §67.2 needs wingmen with orders | Formations, orders, the controller AI and packages **v0.3c**; squadron identity and doctrine v0.4b; persistence v0.6c | §20 tag → "M3 (formations, orders) → M4 (squadrons) → M6 (persistence)" |
| R18 | **Ground warfare** | §21's tag: `M3–M4`. §66.3: ground units and SAM sites in M3, "ground/naval forces" again in M5. Outline: ground targets in phase 3 (§128), ground forces in FP3 (§134) | Targets and SAM sites **v0.3c**; IADS, convoys, infrastructure v0.4b; ground forces at scale v0.5d | §21 tag → "M3 (targets, SAM sites) → M4 (IADS, convoys) → M5 (forces at scale)" |
| R19 | **Factions** | §37's tag: "M6 (data model from M3)". §1.4: four factions in the first product. §26.8 budgets forces by faction doctrine at M4. Old roadmap: four factions at v0.7a | Two factions **v0.3c**; four **v0.4b**; the campaign uses them from v0.6a | §37 tag → "M3 (data model, 2 factions) → M4 (4 in generation) → M6 (campaign)" |
| R20 | **Aircraft and weapon counts** | §66.3 M3: "26-weapon target; M3 delivers the core set of ≥ 10". §67.2: ≥ 5 aircraft, ≥ 10 weapons. §66.6.2: "M3 may ship with 12 weapons instead of 26 … 8 aircraft instead of 12". §1.4: about 12 aircraft and 26 weapons in the first product. Old roadmap: 12 and 26 at v0.7a | 5 aircraft and 13 weapons at v0.3d; 7 and 20 at v0.4; **12 and 26 at v0.5e**; §66.6.2's floor (8 and 12) applies to the beta | §66.6.2: "M3 may ship" → "the first public product (M5) may ship" |
| R21 | **Generator validity** | §66.3 M4 and §71.2: ≥ 99.5 %. Old roadmap v0.4a: ≥ 99 % | **≥ 99.5 %** | — |
| R22 | **Head tracking** | §58.7 sits in a `P1 · M1` chapter, but §66.3's M1 scope names only keyboard, mouse, gamepad and HOTAS; v0.1 did not build it | **v0.4c** | §58 tag → "M1 (devices) → M4 (head tracking)" |
| R23 | **G1(a) blind panel** | §66.4: G1 at the end of M1 with ≥ 12 blind testers. Owner decision D1 (ADR-45): the bots gate v0.1d; the panel stays open as OQ-11 | As D1; section 1.6 raises the same question for the later panels (proposed OQ-12) | §66.4 G1: note ADR-45 |
| R24 | **Durations and team** | §66: estimates for a team of 8–12. OQ-02: provisionally solo. v0.1b–v0.1d each took 3–4 times their slot in engineer-weeks | Estimates kept as relative sizes until OQ-02 is decided | — (OQ-02) |
| R25 | **"New weapon classes" after launch** | §66.7 lists anti-ship, cruise and glide weapons as post-launch classes, but the launch roster already has Skua-1, Tern-2, Dirk-C and GB-500G | M7 adds further weapons of those classes | §66.7: "new weapon classes" → "more weapons in the anti-ship, cruise and glide classes" |

---

# 20. CHANGE LOG

| Version | Date | Change |
|---|---|---|
| 2.1 | 2026-11-12 | Accepted by the owner ([ADR-50](adr/0050-entering-m2.md)): the header status, section 19's rows (amended into GDD 0.2.5), section 1.6 and OQ-12 (automated proxies for every panel), section 3.9's pilot regions, sections 7.0 and 7.1 (OQ-02 closed, OQ-03 decided, OQ-04 kept open with commercial-safe data), section 4.2 (the seams' milestones as GDD 0.2.5 now gives them), sections 18.1, 18.2 and 18.4. Status notes added to v0.0b (closed by the owner's waiver) and v0.1d (tagged), and to 6.6 (the owner's M1 session); v0.2 marked as entered |
| 2.0 | 2026-11-10 | Rewritten in the GDD's format: the reading guide and the human-panel gates (section 1), release channels and modes by version (section 2), the content plan with per-version rosters of aircraft, weapons, units, ships, factions, mission types, twists, events, training, regions, art, audio and localisation (section 3), the system staging matrix and the seams (section 4), systems, content, tests and exit criteria for every sub-milestone (sections 5–15), the M7 track (section 16), cross-cutting tracks (section 17), decisions, cut lines and risks (section 18), and the reconciliation table (section 19). v0.5 is split into five sub-milestones (a–e) so that forces at scale (v0.5d) and the beta (v0.5e) are separate. The finished parts keep their 1.x structure and text word for word (the goal, work list, exit, notes and status of v0.0a, v0.0b and v0.1a–v0.1c, and the version headings), with only section numbers added; v0.0 and v0.1 gain summary tables at the end (5.3, 6.5, 6.6), and v0.1d, in progress, gains a status note for 2026-11-10. The dependency order is kept (2.5) |
| 1.x | 2026-10-14 to 2026-11-09 | The original version-by-version list, with status notes for v0.0b and v0.1a–v0.1d |
