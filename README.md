# clpack

clpack = ChangeLog pack

This is a command line tool for keeping a changelog.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and can be customized.

This tool aims to make change logging as streamlined as possible to encourage developers to do it 
automatically for every bugfix and feature. This can be enforced as part of a CI pipeline.

The entry format is kept simple and readable, so entries can be added manually as well - e.g. if some 
contributors can't or don't want to use this tool. The generated CHANGELOG.md file can be freely edited after clpack
updates it - just keep the header unchanged. 

_clpack is meant to be used with Git, where it can pick up information from branch names. Git is, however, optional. 
You can use this tool with other VCS or none at all._

## Advantages over keeping the changelog manually

- **No merge conflicts to resolve**
    - Each changelog entry (feature, bugfix) lives in its own file until packed into a release.
- **Support for multiple release channels**
    - Normally, you have just one, default, release channel for stable releases - e.g from branch `main` or `master`.
    - However, if you use separate branches for beta, preview, EAP, LTS etc., it is now possible to keep a separate
      changelog file for each, which will be accurate even if fixes are backported, cherry-picked, merged from other
      branches etc.
- **Issue numbers are extracted from branch names**
    - Branches must follow a common pattern (configurable) for this to work, e.g. `1234-awful-crash`,
      `SW-1234-make-ui-more-pretty`
    - GitLab & YouTrack format supported by default
- **Uniform formatting & sections** - a template is pre-generated so developers just fill in the blanks for each
  changelog entry
- **Automatic release dating** - date in configurable format may be added to your changelog file
- **Fully configurable** but also works out of the box for most projects

## Building

clpack uses stable rust. Compile with `cargo build --release`.

The binary is intended to be called `cl` in your path.

## "Getting started"

1. Run `cl init`. Inspect and customize the config file `clpack.toml` as needed.
2. To log a change, on your feature branch, use `cl add` or just `cl` for convenience
3. To pack changelog entries for a release, run `cl pack`.
   - If you use release branches with a common naming scheme, like `rel/3.14`, clpack is able to parse the version
     and use it as a suggestion when asking for version number. The pattern matching is based on regex and is configurable.

Changelog is written into `CHANGELOG.md`. This can be customized as well.

## Minimal setup

The changelog file is not required if you are happy with the defaults.

1. Create folder called changelog in your project
2. Use `cl` in the root of your project. It will use default config and create its subdirectories automatically as needed.

## Adding a changelog entry manually

There is no "vendor lock-in" with clpack. You can simply add changelog entries with your text editor - e.g. 
if you use a machine without the tool, or for contributors using exotic systems like Microsoft Windows
(although clpack, in theory, might be compatible).

**Simply add a Markdown file like `my-bugfix.md` into `changelog/entries/`.**

## Changelog entry formatting

Whether you use clpack or do it manually, the actual entry is always a simple markdown file you edit in your preferred editor.

To set the editor to use, use env variable EDITOR, e.g. vim, nano. You can also create an empty entry template using 
clpack and later edit it in your IDE, just don't forget.

- Empty lines are discarded
- Lines starting with `#` are considered a section name - e.g. Fixes, Improvements. Keep the section names consistent 
  across entries, as they will be grouped when packing the changelog for a release. Lines outside any section will go 
  in the front.
- All other lines will be included in the changelog, without any trimming or changes, and will stay together and in
  the same order -> you can write multi-line entries with indentation.

## Working with release channels

Use this if you need to maintain separate release series, e.g. stable, lts, beta, eap, which share some commits 
(with changelog entries).

Normally, each channel is on its own branch or branches following a naming scheme, e.g. stable releases. This is not mandatory - if it fits
your needs, you can have multiple channels on the same branch, too - or e.g. create a throwaway release channel for each major release, 
so you can keep backporting fixes and making releases from the older branch without merge conflicts in the changelog file.

1. Define channels and branch matching rules in the config file `clpack.toml`. 
   - If there is no matching rule for a channel, i.e. it is selected manually, use empty string
2. When calling `cl pack` and there is more than one channel configured, the channel is auto-detected from the branch name.
   - clpack will ask for channel confirmation, with the auto-detected channel pre-selected
   - You may specify the channel directly by using e.g. `cl pack -x beta`
3. Each channel will have its own changelog file, by default called e.g. `CHANGELOG-BETA.md`

## How it works internally

- Each changelog entry is a markdown file in the folder `changelog/entries`
- clpack maintains JSON files in `changelog/channels` with a list of which entries were included in which release
- Changelog entries stay in their files even after making a release, so if you merge a stable branch into a testing
  branch, you can create a changelog entry for a testing release, and it will include new fixes from stable as well as 
  changes made on the testing branch.
- When a large "epoch" release is made, you can delete (`cl flush` - TODO) the contents of the changelog folder.
  - Do not delete the folder itself, clpack would complain it is missing.  
  - If you have a linear release history without multiple channels or backporting, you can do this after every release.
