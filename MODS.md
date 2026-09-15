# Mods and TORE Fighters

TORE Fighters is licensed under the GPL-3.0.  This document explains what that does and does not mean for people who make content for it.  It is a statement of the project's intent, not an amendment to the license.

## What counts as a mod

A mod is anything the engine loads as data at runtime.  That includes:

- Missions, campaigns, and briefings
- Theaters and terrain packages
- Aircraft and object definitions, flight profiles, and loadouts
- Skins, textures, models, cockpit art, and fonts
- Sound profiles, instrument banks, sound sets, and music
- Scripts, triggers, and configuration files read by the engine
- Anything produced by the TORE Fighters tools and saved as a mod package

## Mods are yours

Mods are not derivative works of the engine.  Loading your content through TORE Fighters does not make it GPL, and the project claims no rights over it.  You choose the license for your mod, or none at all.  You can share it, sell it, or keep it private.

If you want a suggestion, CC-BY-SA 4.0 works well for missions, theaters, and art, because it lets other people build on your work while keeping credit attached.

## What is not a mod

Changes to the engine itself are covered by the GPL-3.0.  That includes:

- Forks and patches of the engine, tools, or client
- Code compiled into the engine or linked against it
- Plugins that ship native code rather than data

If you modify the engine and distribute the result, the GPL requires you to share your source.  That is the point of the license, and it is how improvements make it back to everyone.

The line is simple: if the engine reads it, it is a mod.  If the engine runs it as part of itself, it is the engine.

## Retail assets

TORE Fighters requires a legally owned copy of Jane's Fighters Anthology.  The engine imports assets from the user's own media at runtime.  Nothing derived from retail media is distributed by this project, and mods must follow the same rule.

- Do not include retail files, or files converted from retail files, in a mod package.
- A mod may reference retail assets by name so the engine loads them from the user's own import.
- A mod may replace retail assets with your own original work.

Mods that redistribute retail content will not be listed, linked, or hosted by this project.  If you are unsure whether something is derived from retail media, assume it is.

## Attribution

Please credit the source of any third-party content in your mod, including terrain and imagery data, sound libraries, and models you did not make.  The engine's own attributions are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Mods may say they are made for TORE Fighters.  Mods may not claim to be endorsed by, affiliated with, or an official part of TORE Fighters, Jane's, or Electronic Arts.

## Sharing

There is no official mod repository yet.  Until there is, share mods wherever you like.  When the mod loader lands, packages will carry a manifest with a name, version, author, license, and a list of retail assets they reference, so the engine can check that a user's import covers what the mod needs.

## No warranty

Mods are provided by their authors.  The project does not review, test, or support third-party content, and takes no responsibility for what a mod does to your installation.

## Questions

Open an issue.  If something here is unclear, that is a bug in this document.
