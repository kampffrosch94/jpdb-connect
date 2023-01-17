# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Fixed
- fix for time function related cve on linux. I don't think this was particularly
relevant but I bothered anyways.

## [0.6.0] - 2022-09-07
### Added
- ip option: set the ip address jpdb-connect binds to (default is 127.0.0.1)
### Fixed
- improved documentation of options, don't share your session_id with other people :| 

## [0.5.0] - 2022-09-05
### Added
- auto_forget option: automatically mark added card as unknown
- open a card in browser by clicking "View added note (alt-v)" after adding it in yomichan
- add_mined_sentences option: add example sentence for selected card (thanks @himbosatsu)
- port option: set the port jpdb-connect should listen to (default is 3030 like before)
### Fixed
- improve error message when duplicate detection is on
- auto features not working with kana only cards

## [0.4.1] - 2022-08-21
### Fixed
- boolean config options are now optional

## [0.4.0] - 2022-08-21
### Added
- auto forq
- auto unlock
### Fixed
- improved login check

## [0.3.0] - 2022-08-14
### Added
- validation of configuration (it will tell you if it can't log in)
- logging with log levels, configurable in config file (for debugging purposes)

### Fixed
- error messages will pop up in yomi chan now 

## [0.2.0] - 2022-07-31
### Added
- rate limit for requests
- configuration file
- added option to automatically add cards to a deck

## [0.1.0] - 2022-07-11
### Added
- open jpdb on the appropriate card in the browser
