# Changelog

All notable changes to this project will be documented in this file. See [commit-and-tag-version](https://github.com/absolute-version/commit-and-tag-version) for commit guidelines.

## [0.1.0](https://github.com/snomiao/snocommit/compare/v1.1.0...v0.1.0) (2025-10-24)


### ⚠ BREAKING CHANGES

* mention breaking\n\nThis change introduces a breaking change by modifying the application entry point (src/index.tsx). Downstream consumers may need to update their usage or import paths to align with the new initialization behavior.\n\n[BREAKING CHANGE: The public API exposed by src/index.tsx has changed, requiring downstream code updates to accommodate the new entry point behavior.]

### Features

* add commit logging and TODO spinner note ([9e1bafb](https://github.com/snomiao/snocommit/commit/9e1bafbb42161a774ba571e5062162b5cc0c6f79))
* add tsa-composer ([633f502](https://github.com/snomiao/snocommit/commit/633f502ef3a93eea801f544d89431610959d926d))
* feat: add getRecentCommits helper and include in commit message creation\n\nImplemented getRecentCommits(count) in src/index.tsx to fetch and format recent git commits. Exported the helper for external usage. Updated generateCommitMessage to include a 'Recent Commits' section using the helper. Adjusted tests to verify the default export is a function and that getRecentCommits is exported. Includes error handling: returns '(no recent commits found)' or '(unable to read recent commits)' on failure. ([df24b7e](https://github.com/snomiao/snocommit/commit/df24b7eb7d2e494fc10eacbe1f8247c19e0de5e1))
* Import z-chat-completion and integrate it into commit message generation. Replace getStagedFiles with getStagedFileChanges, update gitDiffMap source, make scopeString mutable, and use zChatCompletion( ([5a0e5b7](https://github.com/snomiao/snocommit/commit/5a0e5b7ff7c284a7a13c029d457e1cab87e96e03))
* openai ([a7ae755](https://github.com/snomiao/snocommit/commit/a7ae7553caea583f06978868db016a85a8edb511))


### Bug Fixes

* allow commit message body without 200-char limit ([4aab473](https://github.com/snomiao/snocommit/commit/4aab4733fbf9495878580906d54d7ddc3f63d73e))
* bypass api key validation\n\nIn src/index.tsx, bypasses validation in validApiKey by returning the provided key directly (using key!). The previous validation logic is commented out. This is a temporary change to bypass API key validation during development and should be revisited to reintroduce proper validation. ([33b33b2](https://github.com/snomiao/snocommit/commit/33b33b22b969eda9728ad44f2cd5e3069d07b6ad))
* format code and fix git commit command ([97adb07](https://github.com/snomiao/snocommit/commit/97adb07b5090f700659a4bd47ef1f49ea41f812f))
* mention breaking\n\nThis change introduces a breaking change by modifying the application entry point (src/index.tsx). Downstream consumers may need to update their usage or import paths to align with the new initialization behavior.\n\n[BREAKING CHANGE: The public API exposed by src/index.tsx has changed, requiring downstream code updates to accommodate the new entry point behavior.] ([da07aec](https://github.com/snomiao/snocommit/commit/da07aec522819e9c0d9952a43446fbb142a34429))
* nano ([49c4751](https://github.com/snomiao/snocommit/commit/49c4751ce2a4a93eb31f7d38684b6976c195f025))
* safemessage ([9f34a31](https://github.com/snomiao/snocommit/commit/9f34a31c743b6bf95882862fadb545dc495e4154))
* validate again\n\n- Re-enabled actual validation in validApiKey: if key is provided and starts with 'sk-' and length > 20, return trimmed key; otherwise return null.\n- Minor formatting adjustments in generateCommitMessage: cleaned up parameter destructuring and string formatting to ensure proper scope handling and newline placement.\n- No breaking changes. ([daf8c11](https://github.com/snomiao/snocommit/commit/daf8c11755503132760e55542da8046c691c68fc))

# 1.7.0 (2023-01-08)

### Bug Fixes

- **[object Promise]:** async scope parse ([04e8c70](https://github.com/snomiao/js/commit/04e8c702cb326c9f0a82fb836ed71713ac2f638d))
- **[object Promise]:** async scope parse ([03cf823](https://github.com/snomiao/js/commit/03cf8234728a254cfe1b235a633df73f0d528473))
- **[object Promise]:** scope : ([047cfef](https://github.com/snomiao/js/commit/047cfefb5cd9a2beb7bd83865a6635cf591e18b1))
- **[object Promise]:** scope : ([4c51668](https://github.com/snomiao/js/commit/4c516680915d9775d738a11ee510dfff64ac2e69))
- **@:** empty scope parse ([6d7c9f9](https://github.com/snomiao/js/commit/6d7c9f9898056f1789c7e07a8116fe93b8b46d13))
- **package:** not to handle npm version cmd error ([496dd56](https://github.com/snomiao/js/commit/496dd5610667764261d05f0de6eab70dc7b327e0))
- **package:** pkgname ([92247c8](https://github.com/snomiao/js/commit/92247c8db659af7e38420b569f86eac47bf26135))
- scope ([d6e7aaa](https://github.com/snomiao/js/commit/d6e7aaa381c8411b8d4d59ffff01427fc0343472))
- scope ([fd07781](https://github.com/snomiao/js/commit/fd0778174a7b667599b18cde7b6201f82f4454da))
- **snocommit:** \_node ([9abd96b](https://github.com/snomiao/js/commit/9abd96b556280ad56a2df8a9a4854ea331ead606))
- **snocommit:** actionCmd after commit ([2efbc08](https://github.com/snomiao/js/commit/2efbc0853657013f735a42ce4c285e7e1b4dce4d))
- **snocommit:** add robust ([18ff419](https://github.com/snomiao/js/commit/18ff41925e6b4b72b47d5a784a77d76c3e2a6d4a))
- **snocommit:** bump version ([c050822](https://github.com/snomiao/js/commit/c0508229371cfc4637a96b132a8af6960a8ed333))
- **snocommit:** chore added ([cfb545c](https://github.com/snomiao/js/commit/cfb545c289d0fafe97b0c39327a86e6bdcc93a21))
- **snocommit:** cwd ([d751a9a](https://github.com/snomiao/js/commit/d751a9aa2d313a070cf11c53c5d1a167019bacca))
- **snocommit:** error notify ([f6f9405](https://github.com/snomiao/js/commit/f6f9405bb026fddf58b3ee9bfea3e7cb72e11d29))
- **snocommit:** git push --tags ([60b1d64](https://github.com/snomiao/js/commit/60b1d64b0bd21ae4f3ef08492ea718f53429306d))
- **snocommit:** git sync anyway ([5b1df5a](https://github.com/snomiao/js/commit/5b1df5a7e7c1f9ed446967af989f353e932a88e7))
- **snocommit:** gitsync_cmd = `git pull ([45323e1](https://github.com/snomiao/js/commit/45323e1ef9d8c311994b35f8c4853049007c0a8a))
- **snocommit:** npm link ([be9a867](https://github.com/snomiao/js/commit/be9a867289327dc1bc411d1bdcb141708d9afaf9))
- **snocommit:** npm link ([8992f47](https://github.com/snomiao/js/commit/8992f4719bfc4e97dfcd83d0f0aa451af85a6810))
- **snocommit:** pkg name ([6df74aa](https://github.com/snomiao/js/commit/6df74aa7ae3040b5209e6994507e0c3a1cbbf3e8))
- **snocommit:** quote for subcommand ([456dc29](https://github.com/snomiao/js/commit/456dc296c20913046e3345b11ea3500cfc5fd2ba))
- **snocommit:** rebuild ([9e8fccd](https://github.com/snomiao/js/commit/9e8fccdf40a16a4f01c6e965617bdf1c5b43d87b))
- **snocommit:** see robust result ([3a871a2](https://github.com/snomiao/js/commit/3a871a25727667e462556defc6fca74e7d12a906))
- **snocommit:** sep path ([39778ac](https://github.com/snomiao/js/commit/39778ac131141a21bdc8277e8ce03214a7c035e4))
- **snocommit:** sep path ([75c8edc](https://github.com/snomiao/js/commit/75c8edc5c443aa3916f4304f0d3086c12da7da45))
- **snocommit:** sep pkg and folder name ([b9774ac](https://github.com/snomiao/js/commit/b9774ac1bc99366a2ad1ca594ef99953f4838c7c))
- **snocommit:** version ([9b7cf88](https://github.com/snomiao/js/commit/9b7cf88e6f42d2eb0bd640abce1328ff03099875))
- **snocommit:** version ([d9196c9](https://github.com/snomiao/js/commit/d9196c9831fb78acbf8c1d1bafea02d78c1a0128))
- **snocommit:** versioning commit ([116b432](https://github.com/snomiao/js/commit/116b4328105baf2776bf3f74e94fc19aed2e9c6d))
- **src:** empty cmd ([95be2fb](https://github.com/snomiao/js/commit/95be2fb02e3a1cd4012f2ed65e53c9cfb4558606))
- **src:** free from versioning Actions ([50bf675](https://github.com/snomiao/js/commit/50bf67515185f961374834dc46e804b728c22f42))

### Features

- **packages:** init ([7006095](https://github.com/snomiao/js/commit/7006095105246ce096e02073bed225c4bc8aa6b8))
- **pkg:** snobuild v3 ([611805b](https://github.com/snomiao/js/commit/611805b3bdf18d8fea6ea5bbe15be2fb5808b6e3))
- **snocommit:** refactor ([11d10bf](https://github.com/snomiao/js/commit/11d10bfacefe080736a589f2c4061ef15e2c22b8))
- **snocommit:** refactor ([249fe71](https://github.com/snomiao/js/commit/249fe7182eb0cab41351a76106a49396cac60c09))
