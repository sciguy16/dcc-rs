# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
### Changed
### Deprecated
### Removed
### Fixed
### Security

## [0.3.0] - 2022-06-15
### Added
* Added service-mode packets
* `PhysicalRegister` has convenience constants allowing the registers to be
referred to by name
### Changed
* Refactored `packets` module to have submodules for each packet class
* Refactored common serialisation code into `packets` module

## [0.2.0] - 2022-06-13
### Added
* Implement `Reset`, `Idle`, and `BroadcastStop` packets
### Changed
* `SpeedAndDirection` packet now uses 28 speed steps
* STM32F103 example is now a speed controller

## [0.1.0] - 2022-06-12
### Added
* Initial implementation that can transmit a `SpeedAndDirection` packet
* Example base station implementation on an STM32F103 "Blue Pill"
