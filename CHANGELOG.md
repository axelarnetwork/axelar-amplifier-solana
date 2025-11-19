# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1]

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- *(its)* Handle source_address as raw bytes ([#81](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/81)) - ([ca2d23c](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/ca2d23c1964392ab8eb2feaa3f086eb20e560ed1))
- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))
- Allow token manager operator to add himself as flow limiter ([#95](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/95)) - ([5807527](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5807527c0841e4f6e0776abd1887b9a792ab5480))
- Don't wrap `RegisterTokenMetadata` in `SendToHub` ([#79](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/79)) - ([e341757](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/e341757f7492b387514cf04a5f88e1c92e0dcebf))
- Its exvul findings 19, 20, 21, 22 ([#75](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/75)) - ([12e6126](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/12e61264e4f3d1dec8c01990dde06b5956259045))
- Check execute deploy interchain token minter validity ([#72](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/72)) - ([25da852](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/25da852699049738ccbc7d0470a314c1bb0091c5))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### 🧪 Testing

- Improve code coverage and test fixtures ([#80](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/80)) - ([38e9135](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/38e91359f336b9379177c192bfe28e9c23dffe1c))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(its)* Remove ITS token approval events ([#82](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/82)) - ([fa4be44](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/fa4be441b63a3f47fc3931b495bd15a288fd0fdc))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @MakisChristou
* @themicp
* @nbayindirli
* @rista404

## [0.1.1]

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Move encoding and gateway commons to `solana-axelar-std` ([#78](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/78)) - ([5236503](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/52365037f9957ef9871100643f6d61ba0304fd9e))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @rista404
* @nbayindirli

## [0.1.1]

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1]

### ⛰️ Features

- *(gateway)* Expect standard recovery IDs on Signatures  ([#74](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/74)) - ([f064294](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/f0642948373085cb058f565d79bb80518f02f5cf))
- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- *(gw17)* Treat `writable` and `signer` as `U256` and validate they are both up to 1 ([#67](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/67)) - ([b02d117](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/b02d1174ac933cbe714177bff88dc7122bd70122))
- *(gw18)* Validate flag in payload anchor deserialize. ([#68](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/68)) - ([cf228bd](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/cf228bd8148e3ed3a2e18708bd0616cbe968f278))
- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))
- Correct merklized typos ([#69](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/69)) - ([a6aa6d8](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/a6aa6d80d88adc8a469f6cd55740bb28e5f0a543))

### 🚜 Refactor

- Move encoding and gateway commons to `solana-axelar-std` ([#78](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/78)) - ([5236503](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/52365037f9957ef9871100643f6d61ba0304fd9e))

### ⚙️ Miscellaneous Tasks

- *(github)* Add release process ([#39](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/39)) - ([0f29d6b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/0f29d6b22a7e96066481fe3609bee02cbddf0917))
- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @themicp
* @rista404
* @nbayindirli
* @Foivos

## [0.1.1]

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1]

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Move encoding and gateway commons to `solana-axelar-std` ([#78](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/78)) - ([5236503](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/52365037f9957ef9871100643f6d61ba0304fd9e))
- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @rista404
* @nbayindirli

## [0.1.1]

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))
- Don't wrap `RegisterTokenMetadata` in `SendToHub` ([#79](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/79)) - ([e341757](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/e341757f7492b387514cf04a5f88e1c92e0dcebf))
- Its exvul findings 19, 20, 21, 22 ([#75](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/75)) - ([12e6126](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/12e61264e4f3d1dec8c01990dde06b5956259045))
- Check execute deploy interchain token minter validity ([#72](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/72)) - ([25da852](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/25da852699049738ccbc7d0470a314c1bb0091c5))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### 🧪 Testing

- Improve code coverage and test fixtures ([#80](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/80)) - ([38e9135](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/38e91359f336b9379177c192bfe28e9c23dffe1c))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(its)* Remove ITS token approval events ([#82](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/82)) - ([fa4be44](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/fa4be441b63a3f47fc3931b495bd15a288fd0fdc))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @themicp
* @MakisChristou
* @rista404

## [0.1.1]

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1]

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1]

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- *(gw17)* Treat `writable` and `signer` as `U256` and validate they are both up to 1 ([#67](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/67)) - ([b02d117](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/b02d1174ac933cbe714177bff88dc7122bd70122))
- *(gw18)* Validate flag in payload anchor deserialize. ([#68](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/68)) - ([cf228bd](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/cf228bd8148e3ed3a2e18708bd0616cbe968f278))
- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))
- Correct merklized typos ([#69](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/69)) - ([a6aa6d8](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/a6aa6d80d88adc8a469f6cd55740bb28e5f0a543))

### ⚙️ Miscellaneous Tasks

- *(github)* Add release process ([#39](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/39)) - ([0f29d6b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/0f29d6b22a7e96066481fe3609bee02cbddf0917))
- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404
* @Foivos

## [0.1.1]

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1]

### 🐛 Bug Fixes

- *(programs)* Add missing 'description' to Cargo.toml ([#90](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/90)) - ([317285b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/317285b184b46678e5037fb8f980c839233da3b3))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1](https://github.com/axelarnetwork/axelar-amplifier-solana/compare/solana-axelar-its-v0.1.0...solana-axelar-its-v0.1.1)

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### 🐛 Bug Fixes

- Don't wrap `RegisterTokenMetadata` in `SendToHub` ([#79](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/79)) - ([e341757](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/e341757f7492b387514cf04a5f88e1c92e0dcebf))
- Its exvul findings 19, 20, 21, 22 ([#75](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/75)) - ([12e6126](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/12e61264e4f3d1dec8c01990dde06b5956259045))
- Check execute deploy interchain token minter validity ([#72](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/72)) - ([25da852](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/25da852699049738ccbc7d0470a314c1bb0091c5))

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### 🧪 Testing

- Improve code coverage and test fixtures ([#80](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/80)) - ([38e9135](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/38e91359f336b9379177c192bfe28e9c23dffe1c))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(its)* Remove ITS token approval events ([#82](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/82)) - ([fa4be44](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/fa4be441b63a3f47fc3931b495bd15a288fd0fdc))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @themicp
* @MakisChristou
* @rista404
* @nbayindirli

## [0.1.1](https://github.com/axelarnetwork/axelar-amplifier-solana/compare/solana-axelar-memo-v0.1.0...solana-axelar-memo-v0.1.1)

### ⛰️ Features

- *(its)* Add deploy, register and execute instructions ([#65](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/65)) - ([d353c7b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/d353c7b256bc2d77c42f4c3e6c3f1169b1837b39))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @rista404
* @nbayindirli

## [0.1.0]

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Add env-based ids to memo and governance ([#63](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/63)) - ([feb070b](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/feb070babfa08062a0d9c49218dcf4d46a760971))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404

## [0.1.1](https://github.com/axelarnetwork/axelar-amplifier-solana/compare/solana-axelar-gas-service-v0.1.0...solana-axelar-gas-service-v0.1.1)

### 🚜 Refactor

- Rm legacy programs and crates ([#62](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/62)) - ([3080fba](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3080fba040d4ab84feff583e6ed92989068b3f9a))

### ⚙️ Miscellaneous Tasks

- *(github)* Use blacksmith runners ([#53](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/53)) - ([5115d6a](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/5115d6a19e22dd04a57a948aba3764e2b6d28fee))
- *(programs)* Devnet-amplifier IDs for solana-12 ([#61](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/61)) - ([6f2d7f1](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/6f2d7f1fc818648e8115220b55a264f73f42ff6b))
- Rename test fixtures to legacy ([#57](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/57)) - ([18222fa](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/18222fabeb7b538fbd20c3ead7510c91b1aff544))
- Rename v1 and v2 programs ([#50](https://github.com/axelarnetwork/axelar-amplifier-solana/pull/50)) - ([3844388](https://github.com/axelarnetwork/axelar-amplifier-solana/commit/3844388fe1e4af25427da160fbd411fe51a4d022))

### Contributors

* @nbayindirli
* @rista404
