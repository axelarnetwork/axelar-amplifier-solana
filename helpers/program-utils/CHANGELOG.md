# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

### ⛰️ Features

- Make governance config fields updatable (w13) ([#204](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/204)) - ([658f238](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/658f238a2b8b1e4cd799bde8ce69a3b031e3016a))
- Put different ids behind feature flags ([#139](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/139)) - ([3896176](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/38961762ed0dba80875697203548f5d744976eaf))
- Fix ACKEE-M1 ([#119](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/119)) - ([509f46e](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/509f46e38cb039dbe1dbf51ab23d88d0713021e0))
- Flatten solana directory ([#98](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/98)) - ([8d681a4](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/8d681a4d92a33fbe321fd921155fab2dd156ef6a))

### 🐛 Bug Fixes

- *(github)* Cargo xtask test ([#59](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/59)) - ([6431f6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6431f6ac15de1c473c74762ce7bf52076c67ad21))
- *(its)* Decouple payer from authority accounts ([#286](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/286)) - ([57b8dbc](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/57b8dbcf1bd46ae2ac20364ed2c5ff8bd5f1dbc3))
- *(its)* [**breaking**] Flow limit unwanted failure ([#229](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/229)) - ([7648abb](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/7648abbd13e73d13ffe83b32cfb3a89622470480))
- C12 ensure all initialization PDA checks deep inspect data ([#291](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/291)) - ([84ffdfa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/84ffdfafb7d62e548eb61eb96f212b8e447b094c))
- Ensure PDA's intialization is checked when closing them ([#205](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/205)) - ([aa47291](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/aa4729171b6206827011fbef440c81f97fea26a3))
- Remove spurious call to max(1) in rent math ([#169](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/169)) - ([6e7d934](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6e7d9345877127a8b18bae1eaa8a3df77e276db6))
- Added checks for sysvars and programs (ACKEE-4) ([#108](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/108)) - ([7fd53a5](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/7fd53a52444de8a8dd4e4478c13b4c5637115a0b))

### 🚜 Refactor

- *(governance)* Account array dedicated types - ([9de30db](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/9de30db77fb77bb3871d8d09b57a4372a04164db))
- *(its)* Migrate to use cpi events - ([675d87b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/675d87b1a8a9c710c2a5e4642ae476bde92a41af))
- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(its)* Remove not addressed TODOs ([#260](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/260)) - ([28b8f0c](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/28b8f0c63a153cfdad6a51c8f955b1451883a5e1))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Fix clippy issues ([#49](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/49)) - ([060c10b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/060c10b46188f713f1b99a469c35c5fe3541b6f3))
- Merge `v2-anchor` into `main` ([#43](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/43)) - ([3594e19](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3594e19da2c5654fb12f18c5450be94bcf056f68))
- Refactor, move utils pda stuff to its own module ([#112](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/112)) - ([c41db4a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/c41db4a0fb9bc647d096b9955f93308fd532c92d))
- Remove rkyv dep from program-utils ([#110](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/110)) - ([bf2c195](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/bf2c195950b891f7b592d4aed0f866ce3eb5fbd2))

### Contributors

* @interoplabs-ci
* @rista404
* @nbayindirli
* @frenzox
* @eloylp
* @pierre-l
* @ICavlek

## [0.1.0]

### ⛰️ Features

- Make governance config fields updatable (w13) ([#204](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/204)) - ([658f238](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/658f238a2b8b1e4cd799bde8ce69a3b031e3016a))
- Put different ids behind feature flags ([#139](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/139)) - ([3896176](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/38961762ed0dba80875697203548f5d744976eaf))
- Fix ACKEE-M1 ([#119](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/119)) - ([509f46e](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/509f46e38cb039dbe1dbf51ab23d88d0713021e0))
- Flatten solana directory ([#98](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/98)) - ([8d681a4](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/8d681a4d92a33fbe321fd921155fab2dd156ef6a))

### 🐛 Bug Fixes

- *(github)* Cargo xtask test ([#59](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/59)) - ([6431f6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6431f6ac15de1c473c74762ce7bf52076c67ad21))
- *(its)* Decouple payer from authority accounts ([#286](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/286)) - ([57b8dbc](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/57b8dbcf1bd46ae2ac20364ed2c5ff8bd5f1dbc3))
- *(its)* [**breaking**] Flow limit unwanted failure ([#229](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/229)) - ([7648abb](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/7648abbd13e73d13ffe83b32cfb3a89622470480))
- C12 ensure all initialization PDA checks deep inspect data ([#291](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/291)) - ([84ffdfa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/84ffdfafb7d62e548eb61eb96f212b8e447b094c))
- Ensure PDA's intialization is checked when closing them ([#205](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/205)) - ([aa47291](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/aa4729171b6206827011fbef440c81f97fea26a3))
- Remove spurious call to max(1) in rent math ([#169](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/169)) - ([6e7d934](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6e7d9345877127a8b18bae1eaa8a3df77e276db6))
- Added checks for sysvars and programs (ACKEE-4) ([#108](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/108)) - ([7fd53a5](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/7fd53a52444de8a8dd4e4478c13b4c5637115a0b))

### 🚜 Refactor

- *(governance)* Account array dedicated types - ([9de30db](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/9de30db77fb77bb3871d8d09b57a4372a04164db))
- *(its)* Migrate to use cpi events - ([675d87b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/675d87b1a8a9c710c2a5e4642ae476bde92a41af))
- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(its)* Remove not addressed TODOs ([#260](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/260)) - ([28b8f0c](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/28b8f0c63a153cfdad6a51c8f955b1451883a5e1))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Fix clippy issues ([#49](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/49)) - ([060c10b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/060c10b46188f713f1b99a469c35c5fe3541b6f3))
- Merge `v2-anchor` into `main` ([#43](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/43)) - ([3594e19](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3594e19da2c5654fb12f18c5450be94bcf056f68))
- Refactor, move utils pda stuff to its own module ([#112](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/112)) - ([c41db4a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/c41db4a0fb9bc647d096b9955f93308fd532c92d))
- Remove rkyv dep from program-utils ([#110](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/110)) - ([bf2c195](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/bf2c195950b891f7b592d4aed0f866ce3eb5fbd2))

### Contributors

* @rista404
* @nbayindirli
* @frenzox
* @eloylp
* @pierre-l
* @ICavlek
